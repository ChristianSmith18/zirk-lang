# Zirk — Standard library specification

## 1. Principles

The stdlib must be small, coherent, typed, cross-platform and explicit about
I/O, permissions and errors. There are no global print or read functions.
Standard modules are imported without quotes:

```text
import { stdout } from std.io;
import { File } from std.fs;
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

`stdin` offers line, char and byte reads. EOF is not an exception: it is
represented through `ReadResult<T>` or an equivalent algebraic result that
distinguishes `Data`, `Eof` and `Error`. Invalid stream states are typed errors;
exceptional system failures may become exceptions per contract.

The async variants suspend a task without blocking a thread.

## 4. `std.fs`

```text
import { File } from std.fs;

mut content: String = match with File.open("data.txt") {
    Ok(file) { file.read_text() }
    Error(error) { return Error(error); }
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

- `Array<T>`, dynamic and general purpose;
- fixed arrays where the size is part of the type;
- `List<T>` where an explicit list contract is needed;
- `Map<K,V>` with hashable/equatable keys;
- `Set<T>`;
- `Range<T>`;
- iterators and views.

Collections offer `map`, `filter`, `reduce`, search, sorting and explicit
conversion. Functional operations do not mutate the source. Index access is
bounds-checked; safe accessors returning an optional type are provided.

## 8. `std.time`

Includes `Duration`, monotonic instants, civil date/time, time zones through
versioned data, timers and cancellable sleep.

```text
await task.sleep(500ms);
await operation timeout 5s;
```

Elapsed measurements use a monotonic clock. Civil date and duration are distinct
types; they are not implicitly mixed.

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
collection and decimal approximation. Every failure shows values, a diff and the
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

## 17. Exclusions

The following are not initially part of the stdlib: DOM/browser, a UI framework,
an ORM, a complete web framework, a package registry client as public API, an
implicit shell, obsolete cryptographic algorithms, and a user-controlled event
loop.
