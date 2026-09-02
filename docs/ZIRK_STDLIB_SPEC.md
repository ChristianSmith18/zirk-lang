# Zirk — Standard library specification

## 1. Principles

The stdlib must be small, coherent, typed, cross-platform and explicit about
I/O, permissions and errors. Print and read operations belong to imported
standard objects; compiler-known convenience members may be called directly
after that object is imported when the name is unambiguous.
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
std.task
std.thread
std.sync
std.parallel
std.collections
std.testing
std.reflect
std.system
```

Implementations may subdivide them without changing their public imports.

`std.terminal` owns `Terminal`, `Style`, `Color`, cursor control, progress and
live regions; `std.io` re-exports the common console-facing types for ordinary
printing. `std.encoding` owns the `Encoding` enum and codecs used explicitly by
text I/O; strict UTF-8 is the default interchange encoding.

## 3. `std.io`

Exposes three streams:

```text
import { stdin, stdout, stderr } from std.io;

stdout.print("Hello", 42);
stdout.println("World", user);
stdout.println();
println("Direct convenience call");
stderr.println("Diagnostic");
mut line = stdin.read_line();
```

- `print(values..., separator: " ")` converts each value through `to_string()`,
  joins them with the separator and writes without a line break.
- `println(values..., separator: " ")` performs the same conversion and adds
  exactly the platform line break. Neither operation has an `end` parameter.
- `println()` writes just a line break.
- `stdout` and `stderr` share the same formatting operations but different
  destinations.
- every printable value uses `to_string(): String` or the corresponding
  formatting trait.
- interpolation is favoured: `stdout.println("User: {user.name}");`.
- an unqualified convenience call resolves through the imported standard object
  when unique; a collision requires `stdout.println(...)`.
- `style` controls terminal presentation while `format` constructs text;
  cursor movement, progress and live regions belong to `Terminal`.
- `write` is the exact text/byte stream primitive and does not insert
  separators, line endings, styles or arbitrary object conversions.
- convenience printing propagates catchable `IoError`; `try_print` and
  `try_println` expose an explicit `Result` for expected output failure.
- `mut print: Fn(String) => Void = stdout.println` creates an independent safe
  callable capability. Compiler-known standard-library extraction performs the
  necessary internal clone/attenuation without exposing mutable authority over
  the original standard object or cloning its native handle.

`stdin` offers byte, Unicode code-point, grapheme and line reads. EOF is not an
exception: `ReadResult<T>` distinguishes `Value(T)`, `End` and
`Error(IoError)`. `read_line` removes the terminator and `Value("")` remains
distinct from `End`. Exact-count reads use `Result` because premature EOF
violates their requested contract.

Blocking operations may offer `_async` variants that return `Task<T>` and
suspend without blocking a thread. There is no `async fn`; inherently
task-aware APIs do not need the suffix.

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
notification is consumed with `await watcher.next()` inside the current task or
a concurrent `task` block; no `async for` construct exists. Overflow is an
explicit event requiring a rescan.

Modes do not use magic strings: they are expressed through options/enums. Errors
distinguish not found, permission denied, already exists, invalid path, wrong
type, EOF where applicable, and system failure.

The filesystem requires read/write permissions with a declarable scope. The
runtime validates real paths where necessary to prevent escapes through `..` or
symlinks.

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

mut result = await Process.run("git", ["status"]);
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
families. Direct absence raises a typed controlled error; `get` returns a typed
`Result`, while `get_or_null` deliberately collapses absence with nullability.
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
remain `Done` after completion. Task-aware streams stay separate so iteration
never hides `await`. Every `Range` has a finite end.

## 8. `std.time`

Exports the sealed temporal family: `Date`, `Time`, `DateTime`, `Instant`,
`ZonedDateTime`, `TimeZone`, signed exact `Duration`, calendar `Period`,
`TimeShift`, weekday/unit enums, DST-resolution policies, clocks, timers and
cancellable sleep. Values are immutable and transformations return new values.

```text
await task.sleep(500ms);
await operation timeout 5s;
```

Elapsed measurements use a monotonic clock. Civil date and duration are distinct
types; they are not implicitly mixed.

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
`TemporalFormatError`. `task.sleep` and timeout APIs require non-negative
`Duration` even though differences can produce signed values.

### 8.1 Native scalar and text contracts

Native numeric types expose documented checked/wrapping/saturating arithmetic,
comparison, conversion and formatting operations. `Float` aliases `Float64`;
there is no valid `NaN`. `Char` is one grapheme and exposes byte/code-point and
Unicode classification methods; `ascii_code(): Int32` returns `-1` when the
grapheme is not exactly ASCII. `String` exposes grapheme `length`, indexing,
slicing, search, replacement, trim/case/split/line/normalization views,
`bytes()`, `codepoints()`, `chars()`, explicit deep `clone()` and conversion.
String mutation follows binding strictness and slice replacement requires equal
grapheme counts.

## 9. `std.task`, `std.thread` and `std.sync`

These modules expose supporting types for the language constructs:

- `Task<T>` handles and scopes;
- `TaskSettlement<T>` with `Fulfilled`, `Rejected`, and `Cancelled`;
- `Task.all`, `Task.first`, `Task.settled`, fair select support, cancellation
  and cancellation reasons;
- bounded channels, rendezvous, defensively limited dynamic channels, closing,
  broadcast/watch/one-shot variants;
- `Mutex<T>`, `RwLock<T>`, `Semaphore`, `Barrier`, and `Once<T>`;
- `Atomic<T>` and memory orderings;
- ordered/unordered parallel operations and deterministic/associative reduction
  primitives;
- `task.blocking` for legacy blocking work.

The `task`, `await`, `parallel` and `thread` syntax belongs to the language; the
stdlib does not create an alternative model.

A `Task<T>` starts immediately and its result is consumed by exactly one
`await`; multiple observers use the corresponding channel family. Cancellation
reason is optional with a default, and scheduling priority is automatic.
Threads cancel cooperatively and are never killed at arbitrary instructions.
Safe `Mutex<T>.with` prevents guard escape and `await`; callback failure releases
the lock without rollback or poisoning. `std.parallel` uses one managed worker
pool, preserves order for map, requires associative reduction, avoids nested
oversubscription and mirrors task failure/settlement cleanup.

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

`permissions.network.connect.origins` and
`permissions.network.listen.addresses` are distinct scopes. Resolutions,
redirects, reconnects, proxies, TLS names, binds, multicast/broadcast, custom
resolvers, local sockets, and interface inspection are validated at their
dynamic boundary. Network waits suspend tasks rather than allocating one thread
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

Every public JSON type uses the uppercase acronym: `JSONValue`, `JSONNumber`,
`JSONCodec<T>`, `JSONLimits`, `JSONPath`, `JSONPointer`, and `JSON*Error`.
`JSONValue` is a mutable in-memory Zirk tree with Null, Boolean, exact
`JSONNumber`, String, Array and insertion-ordered Object variants.

The conversion surface is `JSON.parse` (text to tree), `JSON.stringify` (tree or
typed value to text), `JSON.decode<T>` (text to typed value), `JSON.to_value`
and `JSON.from_value<T>`; there is no redundant `JSON.encode`. Parse is strict
UTF-8 standard JSON, rejects duplicate keys unconditionally, preserves exact
number text, and rejects NaN/infinity. Errors include stable code, location,
path and bounded redacted context. Configurable safe defaults limit bytes,
depth, strings, collection members and number length.

Typed conversion uses compiler-generated or manually implemented ordinary
`JSONCodec<T>` values and never runtime field scanning. Supported class and
attribute decorators configure generated codecs without adding record as a
decorator target or retaining decorator applications. Missing/unknown fields
are errors unless nullable/optional/default or explicitly allowed. Traditional
enums become JSON strings; algebraic enums use discriminated objects; temporal
and byte values require explicit codecs. Constructors and invariants are never
bypassed.

Compact, pretty and canonical output are distinct. Cycles return
`JSONCycleError`. `JSONReader`/`JSONWriter` support bounded streaming and expose
task-aware underlying I/O without hiding `await`. JSON Pointer is included;
JSON5, Schema, advanced JSONPath, Patch, other formats, ORM and domain validation
remain packages.

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

Tests may return `Void`, `Result<Void,E>`, `Task<Void>`, or
`Task<Result<Void,E>>`; the runner awaits task results without `async fn`.
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

`Result<T,E>` provides `is_ok`, `is_error`, nullable extraction, `get_or`,
`get_or_else`, `map`, `map_error`, `and_then`, `or_else`, `unwrap`,
`unwrap_error` and `or_throw`. `Error`, `Throwable`, `RuntimeError`,
`StackTrace`, `Resource<E>`, `ResourceFailure<B,C>`, `SecretString`,
`Environment` and exact preferred alias `Env` are standard contracts.

`Env` exposes static `get`, `get_or_null`, `get_or`, `require`, `get_secret`,
`contains` and `list_names`. Every operation returns `Result` so absence cannot
hide permission, encoding, or platform failure. Generic `get<T>` and `get_or<T>`
use a compile-time-constrained ordinary parsing contract, not reflection, and
distinguish missing, invalid, and unauthorized values.

Every operation checks its named environment or secret grant; broad listing
needs broad authority and returns only visible names, never values. Unauthorized
lookup does not disclose existence. `SecretString` has no general `to_string()`
and can be revealed only to an authorized sink boundary. Environment mutation
is not in the initial API; child configuration belongs to `std.process` and
tests use isolated runner environments.

Filesystem, network and process APIs use scoped permissions and return typed
permission denial. Process execution validates canonical executable and
allowed argument forms; shell execution is separate high-risk authority.

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
