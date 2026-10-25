# Zirk Errors, Resources, and Permissions

Status: **authorial source of truth**, updated 25 October 2026.

This checkpoint defines final failure, deterministic-resource, and authority
semantics independently of implementation phase. Read it after
[`CORE_LANGUAGE_SEMANTICS.md`](CORE_LANGUAGE_SEMANTICS.md). Older
`catch<Type> name`, `compile_permissions`, silent `Result`, close-error, or
manifest-as-consent descriptions are historical when they conflict here.

## 1. Failure taxonomy

| Channel | Meaning | Visible contract | Handling |
|---|---|---|---|
| `Result<T,E>` | expected operational failure | explicit Result context, or `T?` in an ordinary value context | direct `match` for detail; contextual projection when detail is unnecessary |
| `throws X` | extraordinary recoverable failure | declared exception effect | catch or propagate |
| `RuntimeError` | implicit safe-runtime failure | compiler-known operation metadata | optionally catch |
| `fatalError` | irreparable state | `Never` | cannot recover |

There is no implicit conversion between these channels.

## 2. Result

```zirk
enum Result<T, E> {
    Ok(T);
    Error(E);
}
```

`Result` is Zirk's algebraic equivalent of Go's explicit `value, err` outcome,
without permitting invalid “both” or “neither” states. A direct `match` or a
value context explicitly typed `Result<T,E>` preserves both variants. In every
other value-demanding context, `Ok(value)` projects to `value` and `Error(_)`
projects to `null`, producing `T?`. This projection intentionally discards the
error payload. Code that needs the typed failure uses direct `match` or stores
the result in a binding explicitly typed `Result<T,E>`. A Result used as a
standalone expression statement is an effect: `Ok` discards its value, while
`Error` throws its `Throwable` unchanged or wraps another error in a catchable
`NativeError`. Use direct `match` to inspect or handle the error without
throwing. `_ = operation()` deliberately discards both outcomes.

```zirk
map<U>(transform: Fn(T) => U): Result<U,E>
map_error<F>(transform: Fn(E) => F): Result<T,F>
and_then<U>(transform: Fn(T) => Result<U,E>): Result<U,E>
or_else<F>(transform: Fn(E) => Result<T,F>): Result<T,F>
or_throw<X from Throwable>(transform: Fn(E) => X): T throws X
```

The removed status, extraction, fallback, and unwrap methods are replaced by
direct `match` or contextual projection followed by `??`. `Env.get<T>(name)`
projects every `Error`, including permission denial, parse failure, and host
failure, to `null` when used as a value. Use direct `match` whenever those
outcomes must remain distinguishable. If a callback passed to a combinator
declares `throws X`, that effect propagates normally; the combinator does not
turn it into `Error`.

## 3. Errors and exceptions

```zirk
abstract class Error {
    abstract message(): String;
    abstract code(): String;
    abstract cause(): Error?;
    abstract suppressed(): List<Error>;
}

abstract class Throwable implements Error {
    abstract stack_trace(): StackTrace;
}

abstract class RuntimeError implements Throwable { }
```

Any `E` may be used in `Result<T,E>`. Only `Throwable` values can be thrown.
Thrown objects are deeply immutable references with identity through `is` and
no default structural equality.

An explicit exception must be caught or declared:

```zirk
fn synchronize(): Void throws NetworkError | StorageError { ... }
```

Declared exception sets are part of callable compatibility:

```zirk
Fn() => Void throws NetworkError
```

Safety checks such as division by zero, overflow, invalid bounds, missing
direct keys, invalid casts, iterator invalidation, allocation limits, and
invalid runtime state produce typed `RuntimeError` subclasses. They can be
caught but need not be listed in every function signature.

```zirk
try {
    synchronize();
} catch NetworkError.Timeout(duration) {
    retry_after(duration);
} catch StorageError(error) {
    report(error);
} finally {
    metrics.flush();
}
```

Catch uses patterns without guards. Declared exceptions must be caught
exhaustively or propagated; implicit runtime exceptions create no mandatory
catch. Specific handlers precede general ones. `catch Throwable(error)` catches
every recoverable throwable and `catch RuntimeError(error)` only implicit
runtime failures.

`throw;` is legal only in a catch and preserves exact identity, original throw
point, cause, suppressed list, and trace. Wrapping constructs a new exception
with the original as `cause`. Throwing the currently caught binding with
`throw error` is rejected in favor of unambiguous `throw;`.

`finally` always runs but cannot directly `return`, `break`, `continue`,
`throw`, or `fatalError` in a way that replaces an active outcome. If cleanup
fails while another throwable propagates, the original remains primary and the
cleanup error is appended to `suppressed`. Stack traces materialize lazily,
carry source/generated/async context, omit locals and secrets, and have human
and structured forms. An uncaught task exception fails its task; an uncaught
`main` exception emits a diagnostic/trace and exits nonzero. Exceptions must be
translated explicitly at C ABI, process, serialization, and other exception-
free boundaries.

## 4. Deterministic resources

```zirk
contract Resource<E from Error> {
    close(): Result<Void, E>;
    is_closed(): Boolean;
}
```

Acquisition is separate and normally returns `Result<Resource,OpenError>`.
`match with` owns every successful acquisition and closes it exactly once on
normal completion, return, exception, cancellation, break, or continue.

```zirk
import { File, OpenOptions } from std.fs::fs;
import { Path } from std.path::path;

match File.open(Path.of("users.json"), OpenOptions.read_only()) with file {
    Ok(file) => process(file.read_all());
    Error(error) => report(error);
}
```

Grouped acquisition proceeds left-to-right and closes right-to-left. If a
later acquisition fails, every earlier success closes before the error branch.
Nested forms remain valid.

Close failures preserve all outcomes:

```zirk
enum ResourceFailure<BodyError, CloseError> {
    Body(BodyError);
    Close(CloseError);
    BodyAndClose(body: BodyError, close: CloseError);
}
```

Body success plus close failure produces `Close`. Body `Result.Error` plus
close failure produces `BodyAndClose`. During exception propagation the close
failure becomes suppressed on the primary throwable.

Resources cannot escape a managed scope through return, object, collection,
closure, or task unless explicitly transferred by a type satisfying
`TransferableResource`:

```zirk
inmut moved = file.transfer();
file.read_all(); // compile-time use-after-transfer
```

The receiver must close, re-manage, or transfer the responsibility. Resources
do not implement ordinary `Clone`; fallible `duplicate`, `split`, or equivalent
type APIs create separately closable handles. Dependent resources cannot
outlive parents. A non-cloneable resource stored in a container is extracted by
an explicit moving operation such as `take`, not projection-copy indexing.
Provable leaks, duplicate close, escape, and use-after-transfer are compile
errors; dynamic closed/transferred misuse raises typed runtime failure. Runtime
leak reporting is defense, never successful cleanup.

Crossing a task, channel, or thread boundary also requires the compiler-derived
concurrent `Transfer` property. `TransferableResource` expresses the resource's
explicit responsibility handoff; concurrent `Transfer` proves that the complete
handoff is safe across execution contexts. Neither permits aliasing one live
responsibility. Cancellation and task aggregation await resource cleanup before
propagating their final outcome; see
[`STRUCTURED_CONCURRENCY_SEMANTICS.md`](STRUCTURED_CONCURRENCY_SEMANTICS.md).

## 5. Capability requirements

`.zkinit` exposes one finite `requires` block for applications and packages.
It lists capability identifiers only; resource scopes are determined and
checked at the protected operation. No `permissions`, `compile_permissions`,
`during`, build/runtime/test subsection, or per-environment block is part of
the canonical manifest.

```text
requires {
    filesystem.read;
    filesystem.write;
    filesystem.metadata;
    network.connect;
    network.listen;
    network.udp.send;
    network.udp.listen;
    network.local.connect;
    network.local.listen;
    network.inspect_interfaces;
    environment.read;
    environment.list_names;
    secrets.read;
    process.execute;
    process.shell;
}
```

The compiler infers effects through functions, methods, lambdas, closures,
generators, callbacks, and higher-order callables. Effect metadata retains the
protected API, source location, requesting package, and dependency path without
adding permission syntax to `Fn`. Package requirements propagate through the
locked graph; a package requests capability but never grants authority. The
application does not copy every transitive requirement by hand.

A requirement is not user consent. Missing consent does not prevent startup.
At the operation boundary the runtime checks the concrete resource; if denied,
the API returns its ordinary typed error. A preserved `Result` can be handled
with `match`; in an ordinary value context, the existing projection yields
`null`. The operation has no protected host side effect, a warning identifies
capability/resource/requester without secret values, and unrelated execution
continues unless user code stops it.

## 6. Concrete resources and current enforcement

Connect and listen authority are separate. Environment reads and secret reads
are separate. Each request uses a canonical concrete resource, not a broad
implicit grant. Runtime-enforced operations include environment names and
secrets, exact filesystem paths, HTTP destination and configured proxy origins,
DNS answers, redirects, TCP and UDP listener endpoints, UDP destinations,
broadcast/multicast scopes, local socket endpoints, and interface inspection.
OS-assigned port `0` is checked as an ephemeral bind request for the requested
address.

The manifest accepts the finite filesystem and process identifiers. The
compiler-owned `std.fs`/`std.path` subset enforces exact path consent; process
APIs remain unavailable and their identifiers do not create access. Native/FFI
capabilities remain outside this initial vocabulary.

## 7. User-owned consent

Consent is stored outside the repository in a private, user-owned approval
record authenticated with a local key. It binds the canonical project identity,
exact resource scopes, and locked dependency/requester graph including package
version, integrity, and paths. Direct `requires` declarations are excluded from
the project identity digest so adding a missing declaration after a successful
first-use decision does not invalidate that exact resource decision. Changing
the project location or identity, lock selection, requester version/integrity,
or dependency path invalidates the affected record. The approval record's
scopes remain exact; a manifest edit alone never grants authority.

## 8. First-use prompt and audit

At the protected operation, an interactive runtime displays project, operation,
capability, resource, requesting package, and dependency path, then asks
`[y/N]`. Yes saves that exact scope and permits the current operation. For a
direct application call, Zirk creates a minimal `.zkinit` if necessary and adds
a missing `requires` declaration. A transitive dependency's requirement is
not copied into the application's manifest. No denies that operation without
writing manifest or consent. Without a terminal, the runtime denies
immediately, warns, and never waits for input.

The CLI provides:

```text
zirk permissions inspect .
zirk permissions diff .
zirk permissions history .
zirk permissions revoke .
```

`inspect` and `diff` show direct/transitive requirements against exact saved
resources. `history` verifies signed approval/revocation events and records
requester, actor, resource, and time without secrets. `revoke` removes the
project approval before the next protected operation. The former blanket
`permissions approve` action is intentionally unavailable; the CLI directs
the user to the resource-specific prompt.

---

**Previous:** [Core Language Semantics](CORE_LANGUAGE_SEMANTICS.md) · **Next:** [Language Specification](ZIRK_LANGUAGE_SPEC.md)
