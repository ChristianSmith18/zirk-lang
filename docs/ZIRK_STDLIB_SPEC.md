# Zirk — Standard library specification

## 1. Principles

The stdlib must be small, coherent, typed, cross-platform and explicit about
I/O, permissions and errors. It is consumed as ordinary Zirk packages and
published modules. Print and read operations belong to imported standard
objects; published members may be called directly after that module is
imported when the name is unambiguous.
Standard modules are imported without quotes:

```text
import { stdout } from std.io;
import { File } from std.fs;
println("ready"); // Resolves to `stdout.println`.
```

The first version includes the core needed for native applications. Specialized
functionality may ship as official packages without extending the language.

## 2. Initial modules

```text
std.io
std.terminal
std.encoding
std.fs
std.path
std.process
std.text
std.net
std.http
std.json
std.crypto
std.time
std.thread
std.sync
std.collections
std.testing
std.reflect
std.system
```

Task and CPU-parallel operations are language surfaces, not importable
`std.task` or `std.parallel` modules. They are documented here as API areas;
their implementation status is recorded in `ZIRK_FEATURE_STATUS.md`.

Implementations may subdivide them without changing their public imports.
The compiler may map a fully identified standard declaration to its host
implementation internally. This is not source syntax and cannot be requested
by an external package with a matching short name.

This document defines a target library contract; its module list and examples
do not assert current compiler availability. Check
[`ZIRK_FEATURE_STATUS.md`](init/ZIRK_FEATURE_STATUS.md) for delivered modules
and APIs.

`std.terminal` and `std.encoding` are future packages. This delivery owns only
the synchronous `std.io` stream surface; strict UTF-8 is its text encoding.

## 3. `std.io`

Exposes three streams:

```text
import { stdin, stdout, stderr } from std.io;

stdout.print("Hello", "world");
stdout.println("ready");
stderr.println("diagnostic");
match stdin.read_line_result() { ReadResult.End => {}, _ => {} }
```

- `print(...args: Stringifiable)` and `println(...args: Stringifiable)` are
  the delivered output APIs. `Stringifiable` is the standard trait for a
  value with `to_string(): String`, supplied automatically by every
  value-producing type. Its default body is a concrete implementation, so a
  class that declares `to_string()` must mark it `#override`.
  This allows heterogeneous calls such as `stdout.println("count", 2, true)`.
  They join rendered arguments with one space; `println` adds one platform
  line break and `print` adds none.
- `println()` writes just a line break.
- `print()` writes nothing.
- `stdout` and `stderr` share the same formatting operations but different
  destinations.
- an unqualified convenience call resolves through the imported module when
  unique; a collision requires `stdout.println(...)`.
- `flush(): Result<Void, IoError>` is explicit and target-aware.

`IoError` has `Io`, `Decoding`, `Limit`, and `EndOfStream`. `ReadResult<T>` has
`Value(value: T)`, `End`, and `Error(error: IoError)`. `stdin` provides
`read_line(): String?`, `read_line_result(): ReadResult<String>`,
`read_char(): ReadResult<Char>`, `read_bytes(max: UInt64): ReadResult<Array<Byte>>`,
and `read_exact(count: UInt64): Result<Array<Byte>, IoError>`. Empty lines are
values, text decoding is strict UTF-8, and premature exact reads return
`IoError.EndOfStream`. Browser execution retains output capture but rejects
reachable stdin reads before execution. Async streams and terminal APIs are
not delivered by this package.

## 4. `std.fs`

```text
import { File } from std.fs;

mut content: String = match File.open("data.txt") with file {
    Ok(file) => file.read_text();
    Error(error) => return Error(error);
};
```

`File` implements `Resource<FileError>`. `OpenOptions` starts with every flag
false, rejects an option set with no capability, and provides typed presets such
as `read_only`, `write_only`, `read_and_write`, `append`, `create`,
`create_new` and `truncate`. Minimum operations:

- `open`, `create` and opening with typed options;
- `read_text`, `read_bytes`, `read_line`;
- `write_text`, `write_bytes`, `append`;
- `flush`, `metadata` and managed idempotent closing;
- async variants for operations that may wait.

Whole-file reads have size limits; streams/chunks/lines cover large data. Text
defaults to strict UTF-8 while explicit `Encoding` values cover endian-specific
UTF-16/UTF-32, ASCII and Latin-1. Direct and atomic writes are distinct. Append
guarantees apply per call where supported; grouped writes use advisory
`FileLock`. `flush`, `sync_data` and `sync_all` expose progressively stronger
and more expensive buffering/durability boundaries.

The module includes complete file/directory manipulation, metadata, links,
temporary files/directories, shared/exclusive advisory locks and native change
watchers. Recursive operations do not follow symlinks by default. Watcher
notification is consumed through the task-aware API described by its module
contract; watcher delivery is not part of the current compiler surface.
Overflow is an explicit event requiring a rescan.

Modes do not use magic strings: they are expressed through options/enums. Errors
distinguish not found, permission denied, already exists, invalid path, wrong
type, EOF where applicable, and system failure.

The proposed filesystem surface uses separate `filesystem.read` and `filesystem.write` capability requirements and checks canonical resources at the operation boundary. Compiler-owned filesystem APIs are not exposed in this checkout, so these capability identifiers remain reserved and do not grant access.

## 5. `std.path`

```text
import { Path } from std.path;

inmut FILE_PATH = Path("./data/file.txt");
FILE_PATH.parent();
FILE_PATH.name();
FILE_PATH.extension();
```

`Path` is an immutable semantic value, not a `String` or open resource. It
supports
`join`, lexical normalization, components, name, extension, parent,
absolute/canonical through operations that touch the system, and explicit
conversion to a string.

Lexical equality is distinct from the fallible `same_file` query. Native
non-Unicode path data is preserved; strict string conversion may fail and lossy
conversion is explicit. One optimized `Path` type is used rather than a public
`PathBuf` split. Cross-target generation uses explicit path style and dynamic
platform behavior comes from `std.system.Platform`.

It must preserve platform rules and avoid unsafe textual concatenation. Building
or manipulating a path does not touch the filesystem; canonicalizing may, and
requires permission.

## 6. `std.process`

```text
import { Process } from std.process;

mut result = Process.run("git", ["status"]);
```

The API separates executable from arguments; it does not invoke a shell by
default. It offers:

- `run` to obtain task-aware completion and a `ProcessResult`;
- `spawn` to obtain a `ChildProcess` resource;
- configurable stdin/stdout/stderr;
- explicit environment and working directory;
- exit code, signal and captured bytes/text;
- timeout and cooperative cancellation.

Shell execution is a different and visibly dangerous API. It requires the
process permission; extra environment requires its corresponding capability.
Nonzero exit is a successfully observed process result, not an API failure.
Output capture is explicit and bounded; inherited I/O is the CLI-oriented
default, and pipes stream with backpressure. Children inherit only a safe
functional environment unless `inherit_environment()` is explicitly requested.
Scope exit terminates, waits, force-kills after a grace duration if necessary,
reaps and closes handles so no zombie is abandoned. Typed pipelines preserve
each stage result and clean the full structure on cancellation.

## 7. `std.text` and `std.collections`

`std.text` owns `StringBuilder`, checked `format`/`format_dynamic`, linear-time
`Regex`, and Unicode normalization algorithms. Native interpolation remains
`"{expression}"`; formatting uses `:name`, positional `:0`, `\:` escape and
typed `:name|format` specifiers. Regex literals use canonical `re'pattern'` and
dynamic patterns use a fallible parser. Locale-heavy internationalization and
text diff remain packages/tooling.

Minimum types:

- `Array<T>`, always fixed-length, with size inferred from an initializer or
  supplied as `T[n]`/`Array<T>(n)`;
- `List<T>` for a resizable ordered sequence;
- `Map<K,V>` with hashable/equatable keys;
- `Set<T>`;
- `Range<T>`;
- iterators and views.

The family also includes `Deque<T>`, `PriorityQueue<T>`, and restricted
`Queue<T>`/`Stack<T>` interfaces. Map/Set iteration preserves insertion order
while hashing uses a defensive process seed. List growth is automatic;
capacity/reserve/shrink are not public APIs, and exact fixed storage uses
`Array`.

Collections offer `map`, `filter`, `reduce`, search, sorting and explicit
conversion. Functional operations do not mutate the source. Whole collection
assignment shares the container; index, slice, iterator, key/value/entry and
view materialization reads produce deep independent values and require `Clone`
where necessary. A projection used as a mutation place writes original storage.

Index access is bounds-checked and accepts negative positions for ordered
families. Direct absence raises a typed controlled error; collection `get`
returns a typed `Result`; nullable value contexts project that same result to
the value or null. Collections do not expose a parallel `get_or_null` method.
This follows the language-wide contextual projection of `Result<T, E>`.
Slices use direction-sensitive Python-style omitted defaults, reject explicit
out-of-range bounds, deep-copy results and require equal-length replacement.
List resizing uses explicit add/insert/remove/splice APIs; Array never resizes.

Iteration uses `Iterable<out T>`, `Iterator<T>` and
`Iteration<T>.Item/Done`, yields independent values and fails deterministically
after structural invalidation. Read-only `view` is explicit shared storage with
a bounded lifetime; mutable iteration waits for the memory/reference model.
Array/List equality is ordered, Map equality compares mappings, Set equality
compares membership and Range equality compares its definition. Public API
tables document mutation, constraints, complexity and capacity/allocation
errors.

Collection transformations are eager; iterator adapters are lazy and require
explicit `collect` or a typed `to_*` terminal. Iterators are single-pass and
remain `Done` after completion. Streams stay separate so iteration never
implicitly suspends. Every `Range` has a finite end.

## 8. `std.time`

Exports the sealed temporal family: `Date`, `Time`, `DateTime`, `Instant`,
`ZonedDateTime`, `TimeZone`, signed exact `Duration`, calendar `Period`,
`TimeShift`, weekday/unit enums, DST-resolution policies, clocks, timers and
cancellable sleep. Values are immutable and transformations return new values.

```text
Timer.sleep(500ms);
```

Elapsed measurements use a monotonic clock. Civil date and duration are distinct
types; they are not implicitly mixed. There is no general `await` timeout
operator in the available language surface.

Friendly current-time calls live on temporal types (`Instant.now` and
`Date`/`Time`/`ZonedDateTime.now(zone:)`) with an optional injectable clock.
Local-zone discovery is fallible. System, monotonic and virtual test clocks
never mutate the host clock. Timer/Ticker are cancelable resources;
fixed-rate/fixed-delay policies are explicit and ticks report missed intervals.
Cron/calendar scheduling is an official package.

- `Date` validates 1-based calendar components, parses/formats explicit and ISO
  forms, exposes calendar properties, compares chronologically and combines
  with `Time`/`Period`.
- `Time` exposes components through nanoseconds; arithmetic returns `TimeShift`
  when crossing midnight so the day offset is not lost.
- `DateTime` has no zone or absolute timeline meaning until `in_zone` resolves
  it with an explicit ambiguity policy.
- `Instant` supports Unix seconds/milliseconds/nanoseconds, ISO UTC forms and
  exact `Duration` arithmetic.
- `TimeZone` uses versioned IANA identities/rules; a fixed offset is not a zone.
- `ZonedDateTime` preserves its instant when converted to another zone and
  distinguishes exact `Duration` arithmetic from calendar `Period` arithmetic.
- `Duration` supports `ns` through fixed weeks, ISO/compact parsing, components,
  total/whole units, arithmetic, rounding, formatting and humanization.
- `Period` supports years/months/weeks/days, ISO parsing and calendar-context
  conversion; it has no context-free total seconds or ordering.

Minimum controlled errors include `InvalidDate`, `InvalidTime`,
`InvalidDateTime`, `InvalidTimeZone`, `AmbiguousLocalTime`,
`NonexistentLocalTime`, `DurationOverflow`, `TemporalParseError` and
`TemporalFormatError`. `Timer.sleep` and other duration-based timer APIs require
non-negative `Duration` even though differences can produce signed values.

### 8.1 Native scalar and text contracts

Native numeric types expose documented checked/wrapping/saturating arithmetic,
comparison, conversion and formatting operations. `Float` is an exact base-ten
decimal (128-bit coefficient + scale) with no width family, no `NaN` and no
infinity; the IEEE 754 binary family is `Float16`/`Float32`/
`Float64`/`Float128` (`Float` aliases `Float64`), also
with no valid `NaN`. `Char` is one grapheme and exposes byte/code-point and
Unicode classification methods; `ascii_code(): Int32` returns `-1` when the
grapheme is not exactly ASCII. `String` exposes grapheme `length`, indexing,
slicing, search, replacement, trim/case/split/line/normalization views,
`bytes()`, `codepoints()`, `chars()`, explicit deep `clone()` and conversion.
String mutation follows binding strictness and slice replacement requires equal
grapheme counts.

## 9. Concurrency and synchronization APIs

**Available:** structured concurrency uses `concurrent`, `spawn`, `Job<T>` and
`Timer`; cooperative channels provide bounded, rendezvous and growable forms;
`parallel` exposes its implemented pool and collection operations; scalar
`Atomic<T>` is available for target-supported Boolean and signed/unsigned
integer widths through 64 bits. Its payload is inline in the GC-managed object,
pointer atomics are unsupported, operations default to `SeqCst`, and weaker
orders require `unsafe`. A dynamically invalid order raises
`InvalidArgumentError`. The former
`Task<T>`, `task`, `await`, and `select` surface was removed. No general
`task.blocking` bridge or timeout operator is available.

**Planned:** managed `Thread<T>`, `std.sync`, `Mutex<T>`, `RwLock<T>`,
`Semaphore`, `Barrier`, and `Once<T>` remain gated on their compiler, runtime,
ownership, and collector contracts. The runtime's Rust/C `AtomicCell` ABI is
distinct from source `Atomic<T>`.
Runtime combinators such as `settled` likewise do not imply a source-level
standard-library API. `parallel.settled` is not available.

The low-level type family includes `Weak<T>`, `Pointer<T>`,
`NativeSlice<T>`, and `NativeSliceMut<T>`. Weak references upgrade through
`Option<T>`; native views carry checked extent and dependent lifetime. Raw
pointer and weak atomic operations remain governed by unsafe semantics rather
than being made safe merely because a library method exposes them.

## 10. `std.net` and `std.http`

`std.net` provides immutable `IPAddress`/`IPv4Address`/`IPv6Address`,
`HostName`, `Port`, `SocketAddress`, and `NetworkPrefix` values together with
DNS, TCP, UDP, local sockets, network-interface snapshots, and TLS. Both the
`Net.*` namespace and direct imports expose the same symbols; TLS additionally
uses `Net.TLS.*` and `std.net.tls`. IP parsing never performs DNS, host names use
IDNA2008, IPv6 scopes are preserved, and ports validate `0..65535`.

DNS uses the system resolver by default, bounded positive/negative TTL caches,
task-aware cancellation, and typed A/AAAA/CNAME/MX/TXT/SRV/reverse results.
DoT/DoH are explicit configurations. Each resolved IP is revalidated against
the permission grant immediately before each connection attempt, preventing
cached answers or DNS rebinding from escaping scope. Host-name connections use
Happy Eyeballs v2 behavior.

TCP is an ordered byte stream with `ReadResult<Bytes>`, partial `write`,
backpressured `write_all`, half-close, bounded buffers, listeners, and
cancellation-safe reactor waits. UDP preserves datagram source, size, and
boundaries and never silently truncates. Broadcast and multicast are explicit.
TLS defaults to TLS 1.3 with mandatory chain/host validation; TLS 1.2 requires
explicit interoperability configuration and older versions are forbidden.
Custom trust, mTLS, SNI, ALPN, and resumption are typed. No general-purpose
certificate-validation bypass exists.

`.zkinit` declares network capability kinds in the single `requires` list; it has no `permissions.network.*` resource block. The runtime asks for or checks concrete origins, resolved addresses, redirects, reconnects, proxies, TLS names, binds, multicast/broadcast destinations, local sockets, and interface inspection at their dynamic boundaries. Network waits suspend tasks rather than allocating one thread
per socket, and all queues, buffers, attempts, and handshake work have safe
bounds.

`std.http` provides equivalent `HTTP.*` and direct-import surfaces, immutable
`URL`, validated `HTTPHeaders`, integer-backed `HTTPStatus`, one-shot calls,
`HTTPClient.create`, and `HTTPServer`. Dedicated method calls coexist with
`request(HTTPMethod, ...)`. `HTTPMethod.Query`, `client.query`, and
`server.query` implement RFC 10008 QUERY as a safe, idempotent, cacheable method
with required typed content semantics and `Accept-Query` discovery.

Requests accept exactly one of JSON, text, bytes, form, multipart, or streaming
content. Responses expose one consumable body through text, bytes, typed JSON,
or a bounded stream. Headers are case-insensitive, preserve multiple values,
accept lists of pairs, and reject invalid/framing-conflicting fields. Redirects
are bounded and revalidate credentials, permissions, DNS, and TLS at every hop;
cookies require an explicit bounded `CookieJar` on reusable clients.

Clients own isolated origin/proxy/TLS pools, safe retry/cache policy, typed
timeouts, and bounded decompression. Servers offer `route(HTTPMethod, ...)` plus
method conveniences, structured handlers, ordered middleware, streaming
multipart, graceful shutdown, SSE, and WebSocket upgrades. HTTP/1.1 and HTTP/2
share one API; HTTP/3 remains an official QUIC package implementing the same
contracts.

Protocol parsing is strict against request smuggling and response splitting.
All URLs, fields, compressed/decoded bodies, parts, connections, queues,
streams, retries, redirects, handlers, and timeouts have safe bounds. A peer
disconnect cancels owned structured work. No thread is created per request; the
reactor handles I/O, the scheduler runs handlers, and heavy CPU work moves to
`parallel`.

Frameworks, advanced routing, ORM and templating stay in packages, not in the
core.

## 11. `std.json`

The implemented `std.json` surface converts strict JSON text to ordinary Zirk
values. `JSON.parse(document): Dynamic` represents objects as `Object`, arrays
as `List<Dynamic>`, and scalars as their native Zirk types. An internal tagged
number preserves exact JSON number text until checked numeric conversion. There
is no second public JSON object tree.

`JSON.stringify(value: Dynamic): String` serializes JSON-compatible ordinary
Zirk values. It rejects malformed or duplicate-key input, cycles, and runtime
values with no JSON representation through catchable `JsonConversionError`.
Input is limited to 16 MiB and nesting depth 128. Duplicate keys are compared
after escape decoding. Serialization does not return partial output.

Typed conversion is explicit through `JSONCodec<T>`, a structural contract
with `decode(value: Dynamic): T` and `encode(value: T): Dynamic`.
`JSON.decode<T>(document, codec)` parses then delegates validation and
construction to the codec. `JSON.encode(value, codec)` delegates to the codec
then stringifies its ordinary JSON-compatible value. Codecs must call normal
constructors and may validate contract shapes; the runtime does not use
reflection or bypass invariants.

Configurable limits, typed `JSONPath`/`JSONPointer`, generated codecs, streaming
reader/writer APIs, pretty/canonical output, schema validation, and JSON5 are not
part of the implemented surface. The handbook records this boundary; the feature
status catalog remains the implementation evidence.

## 12. `std.crypto`

`Crypto` is the canonical namespace and algorithms may also be imported
directly as the same symbols. Extended acronym names include `SHA_256`,
`AES_256_GCM`, `HMAC_SHA_256`, `ML_KEM_768`, and `ML_DSA_65`. Algorithm-bound
secret/key/nonce/digest/signature/envelope types prevent category confusion;
secrets redact, avoid ordinary clone/string/equality, clear storage where the
target permits, and require explicit duplicate/export operations.

`SecureRandom` uses only the unseedable system CSPRNG and unbiased ranges.
Hashing includes SHA-2/SHA-3 and non-FIPS performance-oriented BLAKE3; MD5/SHA-1
are absent. Password storage defaults to versioned Argon2id with automatic salt,
optional pepper and rehash detection; an explicit FIPS build profile uses
PBKDF2-HMAC-SHA256. HKDF-SHA256/512 and HMAC-SHA256/512 use mandatory context
separation where applicable.

Authenticated encryption only exposes `AES_256_GCM` and
`ChaCha20_Poly1305`, generates nonces automatically, returns a versioned
`SealedMessage<A>`, collapses authentication failure, and frames large streams
without releasing unauthenticated plaintext. Signatures/key establishment use
Ed25519, ECDSA-P256, X25519, explicit RSA-PSS interoperability and finalized
ML-KEM-768/ML-DSA-65/SLH-DSA; protocol layers own hybrid composition.

PKCS#8, SPKI and PEM are standard key formats; JWK uses JSON codecs. KeyStore,
HSM/KMS and X.509/TLS are separate effectful layers. Standard/FIPS profiles
reject disallowed algorithms at build time without runtime substitution.
Family-specific typed errors, constant-time verification, official test vectors,
provider pinning and transparent equivalent hardware acceleration are mandatory.
Pure CPU crypto/CSPRNG needs no permission; storage/network providers carry
their normal authority.

## 13. `std.reflect`

Exposes compiler-provided basic `Type` identity through both `value.type()` and
`T.type()`. `Type` provides value equality, `is(T)`, short and package/module-
qualified diagnostic names, compiler-produced `TypeKind`, immutable generic
arguments, and declared `implements`/`extends` relationships. Constructed
generic types remain distinct even when optimized representations share code.

`Type` does not expose fields, methods, constructors, attributes, decorators,
layout, offsets, dynamic construction, string-named invocation, or dynamic
get/set. Casts remain the language's separate checked operation. Package
versions/integrity and native compatibility are metadata, not parts of the
display name.

Library-defined structural descriptors are ordinary typed values explicitly
generated or authored by a program and reached through ordinary static methods
such as `User.descriptor()`. Each library owns its minimal descriptor shape;
there is no universal descriptor revealing all structure. `std.reflect` does
not discover retained decorator applications and provides no
`Reflection.decorators(...)`. Descriptors do not break visibility or
mutability, grant permissions, or evade public API/compatibility checks.

The compiler Syntax API does not belong to `std.reflect`; it is a separate
tooling API.

## 14. `std.testing`

`assert` and `expect` are explicit imports. Official examples prefer direct,
lower-snake-case `assert` calls; fluent `expect` is available for multiple
readable expectations about one subject. A test should use one vocabulary
consistently. Assertions cover equality, identity, truth, nullability, results,
throwables, collections, ordered sequences, explicit-tolerance floating
approximation, and explicitly grouped `assert.all` failures.

Unit tests use `@test` exclusively in `.spec.zrk`:

```text
// user_service.spec.zrk
@test
fn creates_user(): Void {
    assert.equal(actual, expected);
}
```

Tests run in a runner-owned scope. They may use the delivered
`concurrent`/`spawn`/`Job<T>` surface; the runner joins or cancels owned work
before completing the case.
Optional decorator arguments provide display name, tags, `Duration` timeout,
and compile-time-checked parameter tuples through `cases`.

`@suite` on a test class is the compile-time grouping equivalent of `describe`.
It owns `@before_all`, `@before_each`, `@after_each`, and `@after_all` hooks;
cleanup runs on success, failure, throw, and cancellation with standard composed
failure semantics. `@fixture` defines typed, dependency-checked fixture
producers. Missing/ambiguous fixtures, dependency cycles, invalid hook
signatures, and case arity/type mismatches are compile-time diagnostics.

Using testing decorators outside their recognized files/targets is a compile
error. Tests do not enter release binaries and receive no automatic private
access. General monkey patching and global symbol replacement are excluded;
tests use explicit interfaces, functions, dependencies, fakes, or typed spies.

E2E tests live in `test/*.e2e.zrk` and use `@e2e`. Commands:

```text
zirk test
zirk test --unit
zirk test --e2e
zirk test --all
zirk test --file <path>
zirk test --tag <tag>
zirk test --seed <seed>
zirk test --jobs <n>
zirk test --report human|json|junit
zirk test --update-snapshots
```

Tags filter declared labels; seed reproduces runner-controlled randomness but
never cryptographic randomness; jobs bounds concurrency; reports preserve
stable human/tool/CI formats with secret redaction. Snapshot comparison is
read-only by default and updates only through the explicit flag after showing
diffs. Every failure shows source, actual/expected values, an appropriate diff,
suite/case identity and reproduction seed where applicable.

Tests have exactly ordinary source visibility and no implicit filesystem,
network, process, environment, secret, native, or shell permission. They do not
mutate the real process environment. Runner isolation is provided through
explicit typed fixtures, and permission/sandbox/infrastructure failures remain
distinct from assertion failures.

`@bench` defines benchmarks run by `zirk bench`, with warmup, multiple samples,
statistics, dead-code-elimination prevention and machine-readable output.

## 15. `std.system`

`System` exposes non-sensitive current-process information and operations;
`Platform` exposes read-only host and target descriptors. Platform properties
include `OperatingSystem` (`MacOS`, not a differently cased spelling),
`Architecture`, family, ABI, separators, executable/library extensions, and
Unix/Windows family predicates. `Platform.host` describes the toolchain
execution machine while `Platform.target` describes generated code.

`System.process_id`, arguments, executable and available parallelism are safe
read-only snapshots. `System.current_directory()` is fallible. Detailed CPU,
memory, user, host, device and machine identifiers are not ambient API because
they enable fingerprinting; any specialized access requires a narrow
permission. Process-wide current-directory mutation is excluded.

`System.exit(code)` and directly imported `exit(code)` are the same immediate
boundary operation and do not promise structured cleanup. Normal return from
`main` remains preferred. `Signals.shutdown_events()` provides bounded,
task-aware `Signal.Interrupt`/`Signal.Terminate` events without executing Zirk
inside native signal handlers. The root supervisor always owns shutdown; a
second signal or exhausted deadline forces controlled exit.

## 16. Common contracts

Public stdlib APIs must:

- prefer enums/options over magic strings;
- use `Result` for expected operational failures;
- reserve exceptions for recoverable exceptional failures;
- be cancellable where they may wait;
- document thread-safety, blocking, allocations and permissions;
- accept `Path` in filesystem APIs;
- avoid copies through safe buffers/views where possible;
- offer limits against hostile inputs;
- keep equivalent behaviour across supported targets or declare explicit
  differences.

Intrinsic timeouts are `Duration` parameters, such as
`client.get(url, timeout: 5s)`. A same-named visible identifier can be passed as
the named shorthand `url:`, meaning `url: url`; it is never inferred from a
reordered positional argument.

`Result<T,E>` supports direct variant matching and preserves both variants in
explicitly Result-typed contexts. Other value contexts project it to `T?`, with
`Error(E)` becoming `null`. Its composition operations are `map`, `map_error`,
`and_then`, `or_else`, and `or_throw`. It does not expose status, extraction,
fallback, or unwrap helpers. `Error`, `Throwable`, `RuntimeError`,
`StackTrace`, `Resource<E>`, `ResourceFailure<B,C>`, `SecretString`,
`Environment` and exact preferred alias `Env` are standard contracts.

`Env` exposes static `get`, `require`, `get_secret`, `contains` and `list_names`.
Every operation returns `Result` so explicit matching can distinguish absence,
permission, encoding, or platform failure. Ordinary value use projects any
`Error` to `null`; `Env.get<T>(name) ?? fallback` therefore selects the fallback
for every error variant. Generic `get<T>` uses a compile-time-constrained
ordinary parsing contract, not reflection, and returns `Parse` for malformed
present values.

Every operation checks its named environment or secret grant; broad listing
needs broad authority and returns only visible names, never values. Unauthorized
lookup does not disclose existence. Ordinary formatting and `to_string()` on
`SecretString` yield only `<redacted>`; the value can be revealed only to an
authorized sink boundary. Environment mutation
is not in the initial API; child configuration belongs to `std.process` and
tests use isolated runner environments.

Network and environment APIs use exact operation-boundary consent and return typed permission denial. Filesystem and process capability identifiers are reserved because compiler-owned APIs are not exposed in this checkout; future process execution and shell APIs require separate capability identities and runtime enforcement.

`Function(P...) => R` is the long callable type and `Fn(P...) => R` its
preferred alias. `Clone` means deep logical independence, is required for a
reference-valued projection read, and is never inserted for whole-reference
assignment. Enums are data-only; enum domain transformations are ordinary
external functions, while native name/mapping lookup remains part of the enum
runtime contract.

## 17. Exclusions

The following are not initially part of the stdlib: DOM/browser, a UI framework,
an ORM, a complete web framework, a package registry client as public API, an
implicit shell, obsolete cryptographic algorithms, and a user-controlled event
loop.
