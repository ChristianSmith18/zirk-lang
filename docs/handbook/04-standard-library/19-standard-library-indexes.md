# Standard Library Indexes

This page is a navigation aid, not a second API specification. Follow each link
for signatures, ownership, errors, permissions, cancellation, limits, platform
behavior, and implementation status. When this page and a module chapter differ,
the module chapter and its normative specification own the contract.

> **Concurrency status:** entries marked **delivered** describe compiler/runtime
> capabilities available today. `std.thread` and `std.sync` below are specified
> target APIs, not available Zirk source APIs. Scalar `Atomic<T>` is available
> as a compiler-known source type; it is not evidence that the rest of `std.sync`
> is implemented. See the [feature
> status catalog](../../init/ZIRK_FEATURE_STATUS.md) for the current matrix.

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
| Coordinate concurrent work (**delivered:** `concurrent`, `spawn`, `Job<T>`, channels) | structured-concurrency handbook | `std.sync` remains specified, not implemented |
| Use a dedicated system thread (**specified, not implemented**) | [`std.thread`](09-std-thread.md) | future `std.sync` contract |
| Protect shared state (**specified, not implemented**) | [`std.sync`](10-std-sync.md) | future atomics and memory-safety contract |
| Parallelize CPU work (**delivered**) | [Parallel CPU operations](10a-std-parallel.md) | worker-pool operations and collections |
| Resolve DNS or use TCP, UDP, TLS, or local sockets | [`std.net`](11-std-net.md) | permissions, structured branches |
| Build an HTTP client/server, SSE, or WebSocket endpoint | [`std.http`](12-std-http.md) | `std.net`, `std.json`, `std.crypto` |
| Parse, produce, stream, or bind JSON | [`std.json`](13-std-json.md) | generated codecs, `std.text` |
| Hash, encrypt, sign, derive keys, or generate secure randomness | [`std.crypto`](14-std-crypto.md) | secret handling, TLS |
| Write assertions, suites, fixtures, snapshots, or benchmarks | [`std.testing`](15-std-testing.md) | testing handbook and CLI |
| Compare safe runtime type identities | [`std.reflect`](16-std-reflect.md) | metaprogramming and generated descriptors |
| Inspect host/target or observe shutdown | [`std.system`](17-std-system.md) | `std.process`, permissions |
| Read configuration or secrets from the environment | [`std.environment`](18-std-environment.md) | permissions and `std.process` |

## Module index

| Module or family | Primary public surface | External effect |
| --- | --- | --- |
| `std.io` | `stdin`, `stdout`, `stderr`, `ReadResult<T>` | terminal/stream I/O |
| `std.terminal` | styles, capabilities, cursor/progress/live regions | terminal control |
| `std.encoding` | encoding enum and strict/lossy codecs | none by itself |
| `std.fs` | `File`, options, metadata, directory and watcher operations | filesystem |
| `std.path` | `Path`, `PathStyle`, lexical/system path operations | only system queries |
| `std.process` | `Process`, `ChildProcess`, `ProcessResult`, pipelines | process; shell separately |
| `std.text` | builder, formatting, regex and Unicode text helpers | none |
| `std.collections` | list, array, map, set, queue/deque and iteration contracts | none |
| `std.time` | temporal family, clocks, timers, task-aware sleep | clock/time-zone access where scoped |
| `std.task` | No delivered public module API; concurrent scopes, `Job<T>`, timers and channels use the current compiler/runtime surface | inherited from work |
| `std.thread` | **Specified, not implemented:** dedicated thread creation, join and transfer | planned thread/system capability |
| `std.sync` | **Specified, not implemented:** mutex, read/write lock, semaphore, barrier and once; scalar `Atomic<T>` is available as a language type | shared-state coordination; see atomic limitations |
| CPU parallelism (language surface; no importable `std.parallel` module) | `parallel {}`, `.parallel` collection adapter, `Parallel.each` | inherited from work |
| `std.net` | addresses, DNS, TCP, UDP, TLS, local sockets and interfaces | network connect/listen |
| `std.http` | client/server, URL, headers, bodies, cookies, SSE and WebSocket | network connect/listen |
| `std.json` | exact tree, parse/stringify, typed codecs and streams | none by itself |
| `std.crypto` | CSPRNG, hashes, passwords/KDF, MAC, AEAD, signatures and KEM | providers/storage/network only |
| `std.testing` | assertions, suites, fixtures, snapshots and benchmarks | only effects used by test |
| `std.reflect` | safe `Type` identity and `TypeKind` | none |
| `std.system` | `System`, `Platform`, `Signals` and immediate exit | sensitive queries where scoped |
| `std.environment` | `Environment` / preferred `Env` | environment and secrets |

## Type index

This index lists prominent entry types rather than every helper or enum.

| Type or object | Owner | Purpose |
| --- | --- | --- |
| `ReadResult<T>` | [`std.io`](02-std-io.md) | distinguish `Value`, `End`, and `Error` |
| `File`, `OpenOptions`, `FileMetadata` | [`std.fs`](03-std-fs.md) | managed file access and inspection |
| `Path`, `PathStyle` | [`std.path`](04-std-path.md) | typed lexical and system paths |
| `Process`, `ChildProcess`, `ProcessResult` | [`std.process`](05-std-process.md) | commands and managed child lifetime |
| `StringBuilder`, `Regex` | [`std.text`](05a-std-text.md) | efficient construction and safe matching |
| collection families | [`std.collections`](06-std-collections.md) | owned/reference collections and iterators |
| `Date`, `Time`, `DateTime`, `Instant`, `Duration`, `Period` | [`std.time`](07-std-time.md) | civil and monotonic temporal domains |
| `Channel<T>`, `concurrent`, `Job<T>`, `Timer` | structured-concurrency handbook | delivered structured concurrent work |
| `Thread<T>` | [`std.thread`](09-std-thread.md) | **Specified, not implemented:** explicit dedicated OS thread |
| `Mutex<T>`, `RWLock<T>`, `Semaphore` | [`std.sync`](10-std-sync.md) | **Specified, not implemented:** shared-state coordination |
| `Atomic<T>` | [Atomics](../02-handbook/18-concurrency/13-atomics.md) | Native scalar source API; target-supported Boolean and signed/unsigned integers through 64 bits, GC-inline payload; no pointer atomics |
| `IPAddress`, `HostName`, `Port`, `SocketAddress` | [`std.net`](11-std-net.md) | immutable network addressing |
| `TCPStream`, `TCPListener`, `UDPSocket` | [`std.net`](11-std-net.md) | transport resources |
| `TLSConfiguration` | [`std.net`](11-std-net.md) | authenticated TLS policy |
| `URL`, `HTTPMethod`, `HTTPStatus`, `HTTPHeaders` | [`std.http`](12-std-http.md) | HTTP message identity and metadata |
| `HTTPClient`, `HTTPServer`, `HTTPRequest`, `HTTPResponse` | [`std.http`](12-std-http.md) | HTTP transport/application boundaries |
| `CookieJar`, `SSEResponse`, `WebSocket` | [`std.http`](12-std-http.md) | state, server events and upgraded messages |
| `JSONValue`, `JSONNumber`, `JSONCodec<T>` | [`std.json`](13-std-json.md) | exact JSON and typed conversion |
| `SecretBytes`, key types and `SealedMessage<A>` | [`std.crypto`](14-std-crypto.md) | secret-safe cryptographic material |
| `Type`, `TypeKind` | [`std.reflect`](16-std-reflect.md) | representation-independent runtime identity |
| `System`, `Platform`, `Signal` | [`std.system`](17-std-system.md) | portable system boundary |
| `Environment`, `Env`, `SecretString` | [`std.environment`](18-std-environment.md) | permission-checked configuration |

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
| Network | `IPAddressError`, `HostNameError`, `DNSError`, `TCPError`, `UDPError`, `TLSError`, `LocalSocketError`, `NetworkPermissionError` | resolution, connect, protocol, TLS and denial remain distinct |
| HTTP | `URLError`, `HTTPProtocolError`, `HTTPHeaderError`, `HTTPBodyError`, `HTTPBodyConsumedError`, redirect/cookie/timeout/permission/server variants | status responses are not transport failures |
| JSON | parse, encode/decode, number, limit, path and codec errors | syntax location and typed path are preserved |
| Crypto | random, hash/KDF/MAC, AEAD, signature/KEM, key/provider/profile errors | authentication failure does not reveal why verification failed |
| Testing | `AssertionError`, fixture, timeout, sandbox, infrastructure and permission failures | a failed assertion differs from runner failure |
| Reflection | unavailable generated-descriptor diagnostics | missing structure is not fabricated by `Type` |
| System/environment | `SystemError`, `EnvironmentMissingError`, `EnvironmentParseError`, `EnvironmentPermissionError`, `SecretError` | absence, invalid content and denial remain distinct |

## Permission index

Importing a module grants nothing. Libraries declare `requires`; applications
grant `permissions`; signed developer approval remains external to the repo.

| Capability | Used by | Scope notes |
| --- | --- | --- |
| terminal/input/output | `std.io`, terminal helpers | channel and control capabilities remain explicit |
| filesystem read/write/watch | `std.fs`, snapshots, persisted cookies/keys | canonical path and symlink escape are checked |
| process | `std.process` | executable and allowed argument forms are scoped |
| shell | explicit shell boundary only | separate high-risk grant; never implied by process |
| environment | `Env` ordinary reads | exact names/patterns; denial hides existence |
| secrets | `Env.get_secret`, credential/key sinks | separate from ordinary environment authority |
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
| terminal/file/network/process I/O | follows the behavior documented by that API; the general I/O reactor is not implemented | cancellation cannot preempt a blocking native call |
| blocking operation | runs on the calling thread unless its API documents otherwise | no managed blocking pool or cancellation guarantee is available |
| `concurrent` scope / `Job<T>` wait | **Delivered:** suspends the current branch or consumes its job result | scope joins or cancels owned branches on exit |
| channel send/receive/drain | **Delivered:** send/receive suspend under backpressure; drain receives until closed or cancelled | bounded/rendezvous/growable behavior; `select` is unavailable |
| thread join | **Specified, not implemented:** intended task-aware join contract | no Zirk thread API is currently available |
| mutex/lock acquisition | **Specified, not implemented:** intended synchronization contract | no Zirk lock API is currently available |
| parallel collection work | **Delivered:** managed worker pool | stops scheduling, joins started work, preserves input order where specified |
| DNS/connect/TLS | target contract; current module support is partial | cancellation and transport cleanup depend on the delivered API |
| HTTP body/pool/server | target contract; current language/runtime integration is incomplete | backpressure and structured cleanup are not general runtime guarantees yet |
| timers/sleep | **Delivered:** cooperative timer service | cancellation removes the timer waiter |
| filesystem watcher/signals | specified module contracts; current task-aware delivery is unavailable | no general watcher/signal event flow is exposed |
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
| JSON tree parsing | proportional to input and produced tree within depth/member/byte limits; streaming avoids full-tree allocation |
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
