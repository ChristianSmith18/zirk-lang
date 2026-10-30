# Standard Library Indexes

This page is a navigation aid, not a second API specification. Follow each link
for signatures, ownership, errors, permissions, cancellation, limits, platform
behavior, and implementation status. When this page and a module chapter differ,
the module chapter and its normative specification own the contract.

For the packaging model and the split between reviewable `.zrk` APIs and
compiler-owned host bindings, see the [standard library source and host
boundary](../../development/STDLIB_SOURCE_AND_HOST_BOUNDARY.md).

> **Concurrency status:** entries marked **delivered** describe compiler/runtime
> capabilities available today. `Thread.run` is delivered, while the general
> `Thread` lifecycle API remains specified only. `Mutex<T>.with` and scalar `Atomic<T>` are
> compiler-known source APIs; the other synchronization primitives remain
> specified only. There is no importable `std.sync` package. See the [feature
> status catalog](../../init/ZIRK_FEATURE_STATUS.md) for the current matrix.
>
> `std.environment`, `std.net`, `std.http`, and `std.json` have bundled source
> packages. Native networking previews include permission-checked DNS, TCP,
> UDP, local sockets, interface inspection, authenticated TLS, HTTP/1.1,
> HTTP/2, and optional HTTP/3. The precise target and test scope is recorded in
> the feature-status catalog and each module chapter; object emission and
> emulated runs are not native execution evidence. HTTP
> supports bounded requests and response streaming, pooling, cookies, proxies,
> retries, caching, multipart resources, middleware cancellation, and SSE.
> Remaining gaps include externally triggered long-lived server shutdown and
> debugger/test/structured-output secret redaction. Generic response-producer
> backpressure is covered by HTTP/1.1 and HTTP/2 Unix fixtures and HTTP/2 on
> Windows; the Windows HTTP/1.1 slow-consumer fixture remains Unix-gated. TLS verification is
> intentionally tied to the URL host; custom verifiers and hostname overrides
> are not exposed, and public client-certificate authentication remains
> unavailable. Consult each module
> chapter before relying on a preview API. Managed multipart uploads have
> bounded reads and task-exit cleanup. Server response streaming is available
> through `HTTPResponse.from_stream`. Native runtime tests cover SSE
> slow-consumer backpressure under identity/gzip over HTTP/1.1 and TLS/HTTP/2,
> disconnect cleanup, and task-cancellation root cleanup on the four CI targets.
> Caching is opt-in through `HTTP.get_cached`/`HTTP.request_cached` and currently
> covers bounded fresh GET/HEAD/QUERY responses with `Vary` and conditional
> revalidation. The preview `HTTPClient` provides bounded HTTP/1.1 per-origin
> connection reuse, an opt-in in-memory cookie jar, HTTPS pooling with
> platform trust roots or caller-supplied DER roots, and opt-in bounded retries
> for safe methods on transport/timeout failures. TLS identity is always checked
> against the URL host using platform roots or explicitly supplied DER roots;
> custom verifiers and hostname overrides are not exposed, and public
> client-certificate authentication remains unavailable.
> The cache remains an explicit module-level API by design. Configurable connect, response-header,
> and response-idle timeouts compose with direct, proxy, and custom-root
> clients; compiled-Zirk HTTP tests cover the combinations on macOS and Linux
> ARM64.
> The client advertises and incrementally decodes `br`, `gzip`, and `deflate`
> responses with 16 MiB compressed/expanded bounds, a 1000:1 expansion ceiling,
> and a cumulative two-second codec CPU budget. Buffered and streaming server
> responses also negotiate compression; incoming requests decode one supported
> coding under the same bounds. HTTP/2 reactor response decoding is covered by
> a compressed loopback response test.

## Find an API by task

| Goal | Start here | Related modules |
| --- | --- | --- |
| Read terminal input or write output | [`std.io`](02-std-io.md) | `std.text`, `std.encoding`, `std.terminal` |
| Read, write, watch, or inspect files | [`std.fs`](03-std-fs.md) | `std.path`, `std.encoding`, structured branches |
| Construct or compare paths | [`std.path`](04-std-path.md) | `std.fs`, `std.system.Platform` |
| Run a child program or pipeline | [`std.process`](05-std-process.md) | `std.io`, `std.environment`, structured branches |
| Build, format, search, or match text | [`std.text`](05a-std-text.md) | native `String`/`Char`, `std.encoding` |
| Store and transform in-memory values | [`std.collections`](06-std-collections.md) | iteration and functional-style handbook |
| Work with dates, time zones, clocks, or timers | [`std.time`](07-std-time.md) | temporal-types handbook, structured branches |
| Coordinate concurrent work (**delivered:** `concurrent`, `spawn`, `Job<T>`, channels) | structured-concurrency handbook | `std.sync` has delivered `Mutex<T>.with` and `Atomic<T>`; other synchronizers remain specified only |
| Run a thread body (**delivered:** `Thread.run`) | [`std.thread`](09-std-thread.md) | general `Thread` lifecycle API remains specified only |
| Protect shared state (**partial:** `Mutex<T>.with`, scalar `Atomic<T>`) | [`std.sync`](10-std-sync.md) | read/write locks, semaphores, barriers, once and condition variables remain specified only |
| Parallelize CPU work (**delivered**) | [Parallel CPU operations](10a-std-parallel.md) | worker-pool operations and collections |
| Resolve DNS or use TCP, UDP, TLS, or local sockets | [`std.net`](11-std-net.md) | permissions, structured branches |
| Build an HTTP client/server, SSE, or WebSocket endpoint | [`std.http`](12-std-http.md) | `std.net`, `std.json`, `std.crypto` |
| Parse, produce, query, validate, or bind JSON | [`std.json`](13-std-json.md) | generated codecs; target-matrix evidence |
| Hash, encrypt, sign, derive keys, or generate secure randomness | [`std.crypto`](14-std-crypto.md) | secret handling, TLS |
| Write assertions, suites, fixtures, snapshots, or benchmarks | [`std.testing`](15-std-testing.md) | testing handbook and CLI |
| Compare safe runtime type identities | [`std.reflect`](16-std-reflect.md) | metaprogramming and generated descriptors |
| Inspect host/target or observe shutdown | [`std.system`](17-std-system.md) | `std.process`, permissions |
| Read configuration or secrets from the environment | [`std.environment`](18-std-environment.md) | permissions and `std.process` |

## Module index

| Module or family | Primary public surface | External effect |
| --- | --- | --- |
| `std.io` | `stdin`, `stdout`, `stderr`, `ReadResult<T>` | terminal/stream I/O |
| `std.terminal` | **Specified, not implemented:** styles, capabilities, cursor/progress/live regions | terminal control |
| `std.encoding` | **Specified, not implemented:** encoding enum and strict/lossy codecs | none by itself |
| `std.fs` | Delivered core and advanced APIs: `File`, `Directory`, link and temporary resources, locks, watchers, bounded reads/traversal, progressive and async operations, metadata, atomic replacement | `filesystem.read`, `filesystem.write`, `filesystem.metadata`; watch and lock use operation-specific scopes |
| `std.path` | Partial delivery: immutable `Path`, lexical operations, absolute and canonical queries | `filesystem.read` for absolute paths; `filesystem.metadata` for canonicalization |
| `std.process` | **Specified, not implemented:** `Process`, `ChildProcess`, `ProcessResult`, pipelines | process; shell separately |
| `std.text` | **Partial language surface; no importable package:** regex and selected `String`/`Char` operations are delivered; builder, formatting, and Unicode helpers remain specified | none |
| `std.collections` | Importable bundled source facade for canonical built-in identities: `Array`, `List`, `Map`, `Set`, `Range`, `Deque`, `Queue`, `Stack`, and `PriorityQueue`; recursive aggregate `Hash`/`Equal` combinations and cross-target evidence remain incomplete | none |
| `std.time` | Importable aliases for the compiler-owned `Date`, `Time`, `DateTime`, `Duration`, `Instant`, `Period`, `TimeShift`, `TimeZone`, and `ZonedDateTime` types; zone-host evidence and localized parsing remain in progress | clock/time-zone access where scoped |
| `std.task` | No delivered public module API; concurrent scopes, `Job<T>`, timers and channels use the current compiler/runtime surface | inherited from work |
| `std.thread` | **Partial compiler-known surface; no importable package:** `Thread.run` is delivered; general `Thread<T>` creation, join and transfer remain specified | thread/system capability |
| `std.sync` | **Partial compiler-known language surface; no importable package:** `Mutex<T>.with` and scalar `Atomic<T>` are delivered; read/write lock, semaphore, barrier, once, and condition variable remain specified only | shared-state coordination; see synchronization limitations |
| CPU parallelism (language surface; no importable `std.parallel` module) | `parallel {}`, `.parallel` collection adapter, `Parallel.each` | inherited from work |
| `std.net` | **Native preview:** addresses, DNS, TCP/UDP, local sockets, interface snapshots, and authenticated TLS streams/listeners; consult the module chapter for target evidence and remaining gaps. | network connect/listen |
| `std.http` | **Native preview:** bounded clients/servers, HTTP/1.1, HTTP/2 and optional HTTP/3, URLs, headers, streaming bodies, cookies, SSE, multipart, middleware, and WebSocket paths; consult the module chapter for target evidence and remaining gaps. | network connect/listen |
| `std.json` | **Partial package:** strict JSON and JSON5 parsing, bounded parse/stringify, manual `JSONCodec<T>`, JSON Pointer/JSONPath, pretty and canonical output, Draft 2020-12 validation, incremental text reader, and pull writer backed by bounded output streaming; generated codecs pass native/interpreter cases on all five hosts and emit for all nine target triples in [run 41114248962](https://github.com/ChristianSmith18/zirk-lang/actions/runs/41114248962); a generic bounded sink API remains undelivered | none by itself |
| `std.crypto` | **Specified, not implemented:** CSPRNG, hashes, passwords/KDF, MAC, AEAD, signatures and KEM | providers/storage/network only |
| `std.testing` | **Specified, not implemented:** assertions, suites, fixtures, snapshots and benchmarks | only effects used by test |
| `std.reflect` | **Specified, not implemented:** safe `Type` identity and `TypeKind` | none |
| `std.system` | **Specified, not implemented:** `System`, `Platform`, `Signals` and immediate exit | sensitive queries where scoped |
| `std.environment` | `Environment` / preferred `Env`; typed reads, redacted secrets, and permission-filtered names in lexicographic order | environment and secrets |

## Type index

This index lists prominent entry types rather than every helper or enum.

| Type or object | Owner | Purpose |
| --- | --- | --- |
| `ReadResult<T>` | [`std.io`](02-std-io.md) | distinguish `Value`, `End`, and `Error` |
| `File`, `OpenOptions`, `FileMetadata`, `FileLine`, `FileChunk`, `FileError`, `Directory`, `DirectoryEntry`, `DirectoryEntries`, `DirectoryWalker`, `FileLock`, `FileWatcher`, `TemporaryPath` | [`std.fs`](03-std-fs.md) | bounded, progressive and asynchronous filesystem operations with managed ownership |
| `Path`, `PathError` | [`std.path`](04-std-path.md) | lexical values and fallible host path queries |
| `Process`, `ChildProcess`, `ProcessResult` | [`std.process`](05-std-process.md) | commands and managed child lifetime |
| `StringBuilder`, `Regex` | [`std.text`](05a-std-text.md) | efficient construction and safe matching |
| collection families | [`std.collections`](06-std-collections.md) | owned/reference collections and iterators |
| `Date`, `Time`, `DateTime`, `Duration` | [`std.time`](07-std-time.md) | delivered civil values and elapsed durations |
| `Instant`, `ZonedDateTime`, `TimeZone`, `Period`, `TimeShift` | [`std.time`](07-std-time.md) | `Instant` has checked Unix timestamp factories, RFC 3339 parsing/rendering, ordering, exact `Duration` arithmetic, and native `now()`. Partial `TimeZone`/`ZonedDateTime` APIs support pinned zone parsing, offset and local-time queries, zoned parsing, instant-preserving projection, and explicit DST resolution; locale formatting uses ICU4X 2.4.0 / CLDR 49. `Period` provides component construction, ISO parsing, and formatting. `TimeShift` preserves clock day carry, applies calendar periods before exact durations under explicit DST policies, and has native/interpreter local parity. Localized parsing is delivered, and the named temporal runtime/parity cases passed on all five hosts in [run 41114248962](https://github.com/ChristianSmith18/zirk-lang/actions/runs/41114248962). `std.time` re-exports all nine temporal types as canonical aliases. |
| `Channel<T>`, `concurrent`, `Job<T>`, `Timer` | structured-concurrency handbook | delivered structured concurrent work |
| `Thread<T>` | [`std.thread`](09-std-thread.md) | **Specified, not implemented:** explicit dedicated OS thread |
| `Mutex<T>` | [`std.sync`](10-std-sync.md) | Delivered compiler-known scoped `with` API; no importable `std.sync` package |
| `RWLock<T>`, `Semaphore`, `Barrier`, `Once<T>`, `ConditionVariable` | [`std.sync`](10-std-sync.md) | Specified, not implemented |
| `Atomic<T>` | [Atomics](../02-handbook/18-concurrency/13-atomics.md) | Native scalar source API; target-supported Boolean and signed/unsigned integers through 64 bits, GC-inline payload; no pointer atomics |
| `IPAddress`, `SocketAddress`, `HostPortAddress` | [`std.net`](11-std-net.md) | current immutable address preview |
| `DNS`, `TCPStream`, `TCPListener`, `TCPReadResult<T>` | [`std.net`](11-std-net.md) | current resolver and TCP transport preview, including numeric TCP peer addresses and typed stream reads |
| `TLSStream`, `TLSListener`, `TLSReadResult<T>` | [`std.net`](11-std-net.md) | current authenticated TLS preview with typed stream reads; generic `TLSConfiguration` is not exposed |
| `UDPSocket`, local-socket and interface APIs | [`std.net`](11-std-net.md) | Delivered in the native preview; consult target evidence in the module chapter |
| `URL`, `HTTPMethod`, `HTTPStatus`, `HTTPHeaders` | [`std.http`](12-std-http.md) | HTTP message identity and bounded metadata, including explicitly sensitive fields |
| `HTTPBody`, `HTTPRedirectPolicy`, `HTTPClientOptions`, `HTTPJsonParsable<T>`, `HTTPJsonSerializable<T>`, `decode_json<T>` | [`std.http`](12-std-http.md) | closed request-body representations, opt-in redirect policy, bounded client policy, explicit JSON contracts, and typed JSON decoding |
| `HTTPClient`, `HTTPServer`, `HTTPRequest`, `HTTPResponse`, `HTTPRedirectResult`, `HTTPStreamResponse` | [`std.http`](12-std-http.md) | HTTP transport/application boundaries, opt-in redirect history, and the response compatibility alias |
| `HTTP`, `HTTPProxy`, `HTTPStreamChunk`, `HTTPStreamProducerError` | [`std.http`](12-std-http.md) | one-shot requests, explicit proxying, and bounded stream outcomes |
| `HTTP.get_cached`, `HTTP.request_cached` | [`std.http`](12-std-http.md) | opt-in bounded response cache with freshness and conditional revalidation |
| `HTTPRouter`, `HTTPRouteHandler`, `HTTPRouteParameters`, `HTTPMiddleware`, `HTTPNext`, `HTTPFormData`, `HTTPMultipartData`, `HTTPMultipartPart`, `HTTPMultipartBuilder` | [`std.http`](12-std-http.md) | bounded routing, route callbacks, middleware, URL-encoded forms, parsed multipart parts, and in-memory multipart bodies |
| `CookieJar`, `HTTPEventStream`, `HTTPEvent`, `HTTPServerEvent`, `SSEResponse`, `HTTPWebSocket`, `HTTPWebSocketMessage` | [`std.http`](12-std-http.md) | client events, bounded server event framing, state, and typed WebSocket messages |
| `JSON`, `JSONCodec<T>`, `JSONLimits`, `JsonConversionError` | [`std.json`](13-std-json.md) | ordinary `Object`/`Dynamic` values, RFC 6901/9535 lookup, bounded conversions, schema validation, and JSON5 |
| `SecretBytes`, key types and `SealedMessage<A>` | [`std.crypto`](14-std-crypto.md) | secret-safe cryptographic material |
| `Type`, `TypeKind` | [`std.reflect`](16-std-reflect.md) | representation-independent runtime identity |
| `System`, `Platform`, `Signal` | [`std.system`](17-std-system.md) | portable system boundary |
| `Environment`, `Env`, `EnvironmentParsable<T>`, `SecretString` | [`std.environment`](18-std-environment.md) | permission-checked configuration, compile-time typed parsing, and redacted secrets |

Native language types and their complete member sets are indexed separately in
the [Type Member Index](../11-reference/13-type-member-index.md).

## Error index

All expected operational failures use the module's typed `Result`; the shared
error/resource rules are in [Common API Contracts](01-common-api-contracts.md).

| Area | Principal families | Important distinction |
| --- | --- | --- |
| I/O and encoding | stream, terminal and encoding errors | EOF is `End`, not an error |
| Files and paths | file, directory, path, watch and permission errors | lexical errors differ from system resolution failures |
| Processes | `ProcessError`, shell and permission variants | nonzero child exit is an observed `ProcessResult` |
| Text | format, regex and encoding errors | invalid pattern differs from no match |
| Collections | bounds, mutation/iteration and capacity/limit errors | absence differs from invalid access |
| Temporal | parse, range, zone, ambiguity and clock errors | calendar ambiguity differs from exact-duration failure |
| Tasks/threads/sync | delivered scope/job/channel failures; future thread, lock and blocking error contracts | thread/synchronization error APIs are not currently exposed in Zirk |
| Network | current preview: `AddressError`, `DNSError`, `TCPError`, `TCPResourceError`, `TLSError`, `TLSResourceError` | address, resolution, transport, TLS, resource-close and permission failures remain distinct |
| HTTP | `HTTPError`, `HTTPHeaderError`, `HTTPRouteError`, `HTTPStreamProducerError` | argument, transport/protocol, body, redirect, cookie, timeout, permission and route failures use the delivered enums; status responses are not transport failures |
| JSON | parse, encode/decode, number, limit, path and codec errors | syntax location and typed path are preserved |
| Crypto | random, hash/KDF/MAC, AEAD, signature/KEM, key/provider/profile errors | authentication failure does not reveal why verification failed |
| Testing | `AssertionError`, fixture, timeout, sandbox, infrastructure and permission failures | a failed assertion differs from runner failure |
| Reflection | unavailable generated-descriptor diagnostics | missing structure is not fabricated by `Type` |
| System/environment | `SystemError`, `EnvironmentError`, `SecretError` | missing values, invalid names/encoding, parse failures, permission denial, and platform failures remain distinct |

## Permission index

Importing a module grants nothing. Applications and libraries declare
capability requirements in the single `.zkinit` `requires` list. Developer
consent for exact runtime resources is stored separately outside the repo.

| Capability | Used by | Scope notes |
| --- | --- | --- |
| terminal/input/output | `std.io`, terminal helpers | channel and control capabilities remain explicit |
| filesystem read/write/metadata | `std.fs`, `std.path` | canonical exact-resource grants; full reads include metadata; metadata-only scopes cannot read contents; browser preflight excludes filesystem effects |
| filesystem metadata | `std.fs::Directory.watch` and metadata APIs | a watcher uses an operation-specific metadata scope for an exact directory or descendants; observation does not grant content reads or mutations; browser preflight excludes filesystem effects |
| process | `std.process` | executable and allowed argument forms are scoped |
| shell | explicit shell boundary only | separate high-risk grant; never implied by process |
| environment | `Env` ordinary reads | exact names/patterns; denial hides existence |
| secrets | `Env.get_secret`, HTTPS header credentials, authenticated HTTP proxy credentials | exact secret names remain separate from ordinary environment authority; sinks accept `SecretString` without general string conversion |
| network connect | DNS/TCP/UDP/TLS, HTTP/WebSocket clients | `origins`; resolved IP and every transition revalidated |
| network listen | TCP/UDP/HTTP servers | `addresses`; never implied by connect |
| broad network operations | multicast, broadcast, custom resolver, interface inspection | separately scoped where applicable |
| sensitive system identity | specialized system APIs | no ambient fingerprinting access |
| native/unsafe provider | crypto providers, native libraries, low-level signal APIs | normal unsafe/native rules still apply |
| test effects | E2E, snapshots and effectful fixtures | no implicit test authority; CI fails closed |

See the [Permission Catalog](../11-reference/10-permission-catalog.md) for the
project-level authorization model.

## Waiting, blocking, and cancellation index

| Operation family | Scheduler behavior | Cancellation and cleanup |
| --- | --- | --- |
| `std.fs` methods ending in `_async` | suspends the current task and uses the shared managed-blocking pool | queued work is cancellable; running non-interruptible calls reach a safe completion point; watcher waits unregister on cancellation |
| terminal/file/process I/O without `_async` | follows the behavior documented by that API; the general I/O reactor is not implemented | cancellation cannot preempt a blocking native call |
| blocking operation | runs on the calling thread unless its API documents otherwise | no managed blocking pool or cancellation guarantee is available |
| `concurrent` scope / `Job<T>` wait | **Delivered:** suspends the current branch or consumes its job result | scope joins or cancels owned branches on exit |
| channel send/receive/drain | **Delivered:** send/receive suspend under backpressure; drain receives until closed or cancelled | bounded/rendezvous/growable behavior; `select` is unavailable |
| `Thread.run` | **Delivered:** launches and joins the scoped system-thread body | general thread handles and task-aware join remain unavailable |
| `Mutex<T>.with` | **Delivered:** scoped mutable borrow under the managed cooperative mutex | no public `RwLock`, semaphore, barrier, once, or condition-variable API |
| parallel collection work | **Delivered:** managed worker pool | stops scheduling, joins started work, preserves input order where specified |
| DNS/connect/TLS | **Delivered preview:** bounded DNS/TCP/TLS work suspends the owning task and closes task-owned resources; the 2026-10-22 evidence snapshot has focused native conformance across targets and composite full-suite results | CI evidence is composite across revisions; cancellation and transport cleanup follow `std.net` |
| HTTP body/pool/server | **Delivered preview:** bounded HTTP/1.1 and TLS HTTP/2 requests, streamed bodies, shared clients, finite listener scopes and cancellation paths are implemented | target evidence and the Windows HTTP/3 WebSocket cleanup failure are recorded in the dated feature-status snapshot; see `std.http` for scope-specific guarantees |
| timers/sleep | **Delivered:** cooperative timer service | cancellation removes the timer waiter |
| filesystem watcher | `std.fs::FileWatcher.next_async` suspends the task on supported native targets | bounded queue reports overflow; Windows lock/watch APIs are typed unsupported; browser preflight excludes filesystem effects |
| test case/fixture | target runner contract | runner-scoped timeout and leak handling are not general guarantees yet |

No API spelled as task-aware implies an `async fn` language feature. The
available structured-concurrency surface is `concurrent`, `spawn`, `Job<T>`,
and `Timer`; the former `Task<T>`, `task`, and `await` surface was removed.

## Complexity and allocation index

Complexity depends on the owning representation and platform. Module chapters
state exact guarantees where stable; this table highlights decisions that affect
API choice.

| Operation | Contract |
| --- | --- |
| Path lexical transforms | proportional to path text/components; no system I/O unless explicitly named |
| `StringBuilder` append/finalize | amortized construction intended to avoid repeated whole-string copies |
| List indexing | constant-time for native list representation; projection reads follow clone/value rules |
| Map/set lookup | expected constant time for hash families; ordered/tree families document their own logarithmic behavior |
| Queue/deque end operations | amortized constant time at supported ends |
| Iterator pipelines | lazy unless a terminal operation or concrete collection is requested |
| Temporal exact arithmetic | constant with bounded validation; zone/calendar lookup depends on zone data |
| concurrent-scope/channel operations | bounded scheduler/synchronization work on delivered paths; no public `Task<T>` aggregation API |
| Parallel map | preserves input order and uses one managed bounded worker pool |
| DNS | cache lookup is bounded; network resolution and record count obey limits |
| TCP/HTTP streaming | bounded buffering with backpressure; full-body helpers allocate up to configured limit |
| JSON parsing and streaming | input parsing retains the produced JSON tree but not a second full source-document buffer; the pull writer retains bounded chunks rather than a second full serialized document; both enforce configured depth/member/byte limits |
| Cryptography | algorithm/profile-defined work; password hashing deliberately consumes calibrated resources |
| Reflection identity | does not scan members or retain general structural metadata |
| Test reports/diffs | output and captured values are bounded; truncation is explicit |

## Reading order

New users should read [Common API Contracts](01-common-api-contracts.md), then
the module that owns their task. Authors of libraries should additionally read
the permission and cancellation indexes above. Runtime/compiler implementers
must use the normative specifications linked by each module and must not infer
semantics solely from these summary tables.

---

**Previous:** [← std.environment](18-std-environment.md) · **Next:** [Native and Low-Level →](../05-native-and-low-level/README.md)
