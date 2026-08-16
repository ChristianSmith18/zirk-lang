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
std.fs
std.path
std.process
std.net
std.http
std.json
std.crypto
std.time
std.task
std.thread
std.sync
std.collections
std.testing
std.reflect
std.system
```

Implementations may subdivide them without changing their public imports.

## 3. `std.io`

Exposes three streams:

```text
import { stdin, stdout, stderr } from std.io;

stdout.print("Hola");
stdout.println("Mundo");
stdout.println();
println("Direct convenience call");
stderr.println("Diagnóstico");
mut line = stdin.read_line();
```

- `print(value)` writes without a line break.
- `println(value)` writes with the platform line break.
- `println()` writes just a line break.
- `stdout` and `stderr` share the same formatting operations but different
  destinations.
- every printable value uses `to_string(): String` or the corresponding
  formatting trait.
- interpolation is favoured: `stdout.println("User: {user.name}");`.
- an unqualified convenience call resolves through the imported standard object
  when unique; a collision requires `stdout.println(...)`.
- `mut print: Fn(String) => Void = stdout.println` produces a callable bound to
  `stdout` without cloning the stream or native handle; cloning the callable
  preserves the receiver unless the receiver is explicitly cloned first.

`stdin` offers line, char and byte reads. EOF is not an exception: it is
represented through `ReadResult<T>` or an equivalent algebraic result that
distinguishes `Data`, `Eof` and `Error`. Invalid stream states are typed errors;
exceptional system failures may become exceptions per contract.

The async variants suspend a task without blocking a thread.

## 4. `std.fs`

```text
import { File } from std.fs;

mut content: String = match File.open("data.txt") with file {
    Ok(file) => file.read_text();
    Error(error) => return Error(error);
};
```

`File` implements `Resource<FileError>`. Minimum operations:

- `open`, `create` and opening with typed options;
- `read_text`, `read_bytes`, `read_line`;
- `write_text`, `write_bytes`, `append`;
- `flush`, `metadata` and managed idempotent closing;
- async variants for operations that may wait.

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

`Path` is a semantic representation of paths, not a `String`. It supports
`join`, lexical normalization, components, name, extension, parent,
absolute/canonical through operations that touch the system, and explicit
conversion to a string.

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

- `run` to await a result;
- `spawn` to obtain a `ChildProcess` resource;
- configurable stdin/stdout/stderr;
- explicit environment and working directory;
- exit code, signal and captured bytes/text;
- timeout and cooperative cancellation.

Shell execution is a different and visibly dangerous API. It requires the
process permission; extra environment requires its corresponding capability.

## 7. `std.collections`

Minimum types:

- `Array<T>`, always fixed-length, with size inferred from an initializer or
  supplied as `T[n]`/`Array<T>(n)`;
- `List<T>` for a resizable ordered sequence;
- `Map<K,V>` with hashable/equatable keys;
- `Set<T>`;
- `Range<T>`;
- iterators and views.

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
- cancellation and cancellation reasons;
- bounded/unbounded `Channel<T>` and closing;
- `Thread<T>` and `join`;
- `Mutex<T>`, read/write locks, semaphores and barriers where justified;
- `Atomic<T>` and memory orderings;
- parallel reduction primitives.

The `task`, `await`, `parallel` and `thread` syntax belongs to the language; the
stdlib does not create an alternative model.

## 10. `std.net` and `std.http`

`std.net` provides addresses, DNS, TCP and UDP through typed, cancellable APIs
compatible with the reactor. Every connection respects `permissions.network`.

`std.http` initially includes a basic native client and server, with:

- typed request/response;
- headers with validation;
- streaming and backpressure;
- configurable timeouts and limits;
- TLS through an audited implementation;
- cancellation tied to disconnection;
- handlers executed as structured tasks.

No thread is created per request. The reactor handles I/O and the scheduler runs
handlers; heavy CPU work must move to `parallel`.

Frameworks, advanced routing, ORM and templating stay in packages, not in the
core.

## 11. `std.json`

Offers a typed JSON tree and generic encode/decode. Serializer derivation may be
done through decorators or requested reflection. Errors include location, path
and expected type. Depth/size limits must prevent hostile consumption.

## 12. `std.crypto`

Only modern, audited algorithms, with safe defaults, constant-time comparison
where appropriate, the system CSPRNG and types that make it hard to mix keys,
nonces and hashes. Obsolete algorithms are not enabled for convenience. The APIs
may be backed by verified native libraries.

## 13. `std.reflect`

Exposes basic type identity, always available, and structural metadata only
where it was preserved. It does not allow breaking visibility or mutability.
Dynamic reflection requiring absent metadata produces an explicit
result/diagnostic.

The compiler Syntax API does not belong to `std.reflect`; it is a separate
tooling API.

## 14. `std.testing`

Unit tests use `@test` exclusively in `.spec.zrk`:

```text
// user_service.spec.zrk
@test
fn creates_user(): Void {
    assert.equal(actual, expected);
}
```

Using `@test` outside `.spec.zrk` is a compile error. Tests do not enter release
binaries and receive no automatic private access.

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
zirk test --report json
```

Minimum assertions: equality, identity, truth, nullability, result, exception,
collection and floating approximation. Every failure shows values, a diff and the
source location.

`@bench` defines benchmarks run by `zirk bench`, with warmup, multiple samples,
statistics, dead-code-elimination prevention and machine-readable output.

## 15. `std.system`

Exposes portable information about the process, target and signals without
turning internal runtime details into stable API. `exit(code)` is immediate and
should be reserved for boundaries; returning normally from `main` allows an
ordered shutdown.

Environment variables, signals and sensitive data require permissions according
to their capability.

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

`Result<T,E>` provides `is_ok`, `is_error`, nullable extraction, `get_or`,
`get_or_else`, `map`, `map_error`, `and_then`, `or_else`, `unwrap`,
`unwrap_error` and `or_throw`. `Error`, `Throwable`, `RuntimeError`,
`StackTrace`, `Resource<E>`, `ResourceFailure<B,C>`, `SecretString`,
`Environment` and exact preferred alias `Env` are standard contracts.

`Env` exposes static `get`, `get_or_null`, `get_or`, `require`, `get_secret`,
`contains` and `list_names`. Every operation checks its named environment or
secret grant; broad listing needs broad authority. Secrets redact from
diagnostics, logs and traces and require deliberate reveal. Environment
mutation is not in the initial API.

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
