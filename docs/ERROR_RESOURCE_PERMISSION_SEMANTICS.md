# Zirk Errors, Resources, and Permissions

Status: **authorial source of truth**, accepted 16 August 2026.

This checkpoint defines final failure, deterministic-resource, and authority
semantics independently of implementation phase. Read it after
[`CORE_LANGUAGE_SEMANTICS.md`](CORE_LANGUAGE_SEMANTICS.md). Older
`catch<Type> name`, `compile_permissions`, silent `Result`, close-error, or
manifest-as-consent descriptions are historical when they conflict here.

## 1. Failure taxonomy

| Channel | Meaning | Visible contract | Handling |
|---|---|---|---|
| `Result<T,E>` | expected operational failure | return type | mandatory |
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
without permitting invalid “both” or “neither” states. A result is handled by
exhaustive `match` or a method. Ignoring it is a compile-time error; deliberate
discard is `_ = operation()` and may receive a linter warning without a
justifying comment.

```zirk
is_ok(): Boolean
is_error(): Boolean
ok_or_null(): T?
error_or_null(): E?
get_or(default_value: T): T
get_or_else(factory: Fn() => T): T
map<U>(transform: Fn(T) => U): Result<U,E>
map_error<F>(transform: Fn(E) => F): Result<T,F>
and_then<U>(transform: Fn(T) => Result<U,E>): Result<U,E>
or_else<F>(transform: Fn(E) => Result<T,F>): Result<T,F>
unwrap(): T
unwrap_error(): E
or_throw<X from Throwable>(transform: Fn(E) => X): T throws X
```

`unwrap` on the wrong variant invokes `fatalError` with the source location,
found variant, safe value representation, and handling guidance. If a callback
passed to a combinator declares `throws X`, that effect propagates normally;
the combinator does not turn it into `Error`.

## 3. Errors and exceptions

```zirk
abstract class Error {
    abstract fn message(): String;
    abstract fn code(): String;
    abstract fn cause(): Error?;
    abstract fn suppressed(): List<Error>;
}

abstract class Throwable implements Error {
    abstract fn stack_trace(): StackTrace;
}

abstract class RuntimeError implements Throwable {}
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
interface Resource<E from Error> {
    fn close(): Result<Void,E>;
    fn is_closed(): Boolean;
}
```

Acquisition is separate and normally returns `Result<Resource,OpenError>`.
`match with` owns every successful acquisition and closes it exactly once on
normal completion, return, exception, cancellation, break, or continue.

```zirk
match File.open("users.json") with file {
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

## 5. Permission declarations

Zirk exposes only two authority blocks:

- `requires`: a library requests capabilities but grants none.
- `permissions`: an application grants capabilities to declared requesters.

Each operation declares `during: build`, `runtime`, or `both`; there is no
public top-level `compile_permissions` block.

```zirk
permissions {
    filesystem {
        read: { paths: ["./config/**"]; during: runtime; }
    }
    network {
        connect: { origins: ["https://api.example.com:443"]; during: runtime; }
    }
}
```

The compiler infers effects through privileged calls, functions, methods,
lambdas, closures, generators, and higher-order callables. Permission syntax is
not added to `Fn`, but compiler/callable metadata retains the requirement and
diagnostics show the call path to the privileged API. `init.zrk` is the only
grant location.

## 6. Scoped authority

- Filesystem grants distinguish read/write and canonical path patterns; checks
  resolve `.`/`..`, enforce symlink policy, and prevent validation/use races.
- Network grants restrict protocol, host, port, redirects, DNS results, and
  reconnects. Leaving the authorized origin requires another grant.
- Process grants restrict the canonical executable and optionally argument
  forms. `arguments: all` warns. Shell execution is a separate critical grant.
- Environment grants restrict names and phase. Secrets use `SecretString`, are
  never logged or traced, and require deliberate reveal at an authorized
  boundary.

```zirk
Environment
Env // exact preferred alias

Env.get(name): Result<String,EnvironmentError>
Env.get_or_null(name): Result<String?,EnvironmentError>
Env.get_or(name, fallback): Result<String,EnvironmentError>
Env.require(name): Result<String,EnvironmentError>
Env.get_secret(name): Result<SecretString,SecretError>
Env.contains(name): Result<Boolean,EnvironmentError>
Env.list_names(): Result<List<String>,EnvironmentError>
```

Every environment operation returns `Result` so absence or a fallback cannot
hide permission, encoding, or platform failure. `Env.list_names` requires broad
environment-read authority. Environment writes are excluded initially. A
dynamic target outside the effective grant returns the privileged operation's
typed `PermissionDeniedError`; deployed programs never prompt or modify their
own manifest.

## 7. Consent is not source text

`init.zrk` states what is requested. Consent lives outside the repository in a
signed Zirk approval record protected by the OS key store. It binds:

- project name and absolute canonical location;
- OS user and device;
- exact permission scopes and build/runtime phases;
- requesting dependency names, versions, integrity, and transitive paths;
- approval timestamp and Zirk version.

Moving or renaming a project requires new consent. Widening a grant, adding or
updating a requester, changing integrity/path/phase, or invalidating approval
also requires consent even if textual scopes look unchanged. Removing or
narrowing authority does not. Editing `init.zrk`, copying another approval,
tampering with storage, or deleting it grants nothing; failure reconstructs
analysis or asks again. Build code has no public access to approval storage.

## 8. Incremental approval and tooling

Every authority-bearing `zirk build`, `run`, `test`, or equivalent command
compares signed project-location, manifest, permission, lockfile, requester-
subgraph, and approval fingerprints. Exact matches take a constant-time fast
path. A change recomputes only affected graph segments.

An interactive trusted CLI shows the exact grant, phase, call path, requester,
dependency path, and manifest diff, then offers allow once, approve the exact
set, or deny. Approval edits `init.zrk` with its parser/formatter and records a
signature. Dynamic scopes require manual patterns. `all` is legal only after a
critical warning and typing the project name; generic `--yes` cannot approve
it. CI uses protected explicit policy and fails noninteractively on widening.

```text
zirk permissions show
zirk permissions diff
zirk permissions approve
zirk permissions revoke
zirk permissions history
```

History records who approved which project/location/requester/scope/phase and
when, without secrets. Revocation applies before the next privileged action.

---

**Previous:** [Core Language Semantics](CORE_LANGUAGE_SEMANTICS.md) · **Next:** [Language Specification](ZIRK_LANGUAGE_SPEC.md)
