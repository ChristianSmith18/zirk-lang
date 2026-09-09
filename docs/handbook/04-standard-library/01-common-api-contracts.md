# Common API Contracts

The standard library follows one set of operational rules across files,
processes, network connections, timers and every other system boundary. These
rules are part of the public contract: an implementation cannot replace a
typed failure with a silent fallback, abandon a child operation after
cancellation or acquire authority that the application did not grant.

> **Implementation status:** this chapter specifies the accepted Zirk 1.x API
> contract. Individual modules may be ahead of the current compiler and runtime;
> consult [Feature Status](../11-reference/12-feature-status.md) before assuming
> that an example executes in the current build.

## Failure channels

Expected operational failures use `Result<T,E>`. A missing file, denied
permission or unavailable executable is expected in the sense that an
application can describe a normal recovery path. Ignoring a `Result` is a
compile-time error.

```zirk
match File.read_text(path) {
    Ok(content) => parse(content),
    Error(FileError.NotFound(_)) => use_defaults(),
    Error(error) => report(error),
}
```

Extraordinary recoverable failures use `throws`; compiler-known safety and
runtime failures remain catchable without being repeated in every signature.
Zirk 1.x has no `Result` propagation operator: `?` is reserved for nullable
types. Use exhaustive `match`, a `Result` combinator or `or_throw`.

## Typed configuration

Public APIs prefer enums, option records and named arguments over magic strings
or positional Boolean clusters:

```zirk
File.open(
    path:,
    options: OpenOptions.read_and_write().with_create(true),
);
```

A named argument can abbreviate `name: name` as `name:`:

```zirk
inmut url = "https://api.example.com";
inmut timeout = 5s;

client.get(timeout:, url:);
// Equivalent to client.get(timeout: timeout, url: url).
```

The shorthand accepts a simple visible identifier only. It may be mixed with
positional and explicit named arguments, but after the first named argument all
remaining arguments must also be named. Repeated labels and missing variables
are compile-time errors.

## Waiting, tasks and timeouts

There is no `async fn` and no public event loop. An operation that can suspend
returns `Task<T>` or another standard awaitable; `await` suspends the current
task without reserving an operating-system thread.

When a blocking and a task-aware API coexist, the latter uses `_async`:

```zirk
inmut immediate = File.read_text(path);
inmut pending = File.read_text_async(path);
inmut content = await pending;
```

Inherently task-aware APIs such as channel receive and watcher notification do
not need the suffix:

```zirk
inmut event = await watcher.next();
```

Timeouts are typed `Duration` arguments when they are intrinsic to the
operation:

```zirk
client.get(url, timeout: 5s);
process.wait(timeout: 30s);
```

The language-level `await operation timeout 5s` remains available when the
caller must impose a boundary on an arbitrary task. Expiry requests
cancellation and awaits cleanup before producing `TimeoutError`; it never
abandons work in the background.

## Resources

Handles with deterministic cleanup implement `Resource<E>`:

```zirk
interface Resource<E from Error> {
    close(): Result<Void,E>;
    is_closed(): Boolean;
}
```

`close()` is public and idempotent. Managed acquisition closes exactly once on
normal completion, return, exception, cancellation, `break` or `continue`:

```zirk
match File.open(path, OpenOptions.read_only()) with file {
    Ok(file) => consume(file),
    Error(error) => report(error),
}
```

Using a dynamically closed resource produces `ResourceClosedError`. Resources
cannot escape a managed scope unless their type supports explicit transfer;
they never gain an ordinary cloning contract merely for convenience.

## Safe callable extraction

Extracting a callable from an ordinary object follows the language's reference
and projection rules. Compiler-known standard-library objects additionally
attenuate the capture: they create an independent safe callable capability
without exposing mutable control over the original standard object.

```zirk
mut log = stdout.println;
log("ready");
```

This is conceptually a safe internal clone of the operation, not a clone of the
native stream handle and not shared mutable authority over `stdout`.

## Limits, allocation and platform behavior

Any API that may consume attacker-controlled memory, recursion depth, handles
or time exposes a typed limit or a documented safe default. Reaching a limit
produces a descriptive typed failure; it never silently truncates security-
relevant data. Streaming APIs exist when a bounded whole-value API would be
inappropriate.

Individual calls document whether they block, suspend, allocate, are thread-
safe and require permission. Supported targets behave equivalently unless the
module publishes a platform-specific guarantee. Platform differences are data
available through `std.system.Platform`, not guessed from path strings or
environment variables.

## Permissions

Importing a module grants nothing. Privileged operations carry inferred effects
through functions, closures and tasks, and `.zkinit` is the only application
grant location. Denial is a typed operational result. Filesystem checks use
canonical paths and symlink-aware policy; process checks use the canonical
executable and permitted argument forms.

The normative owners are [Errors, Resources, and Permissions](../../ERROR_RESOURCE_PERMISSION_SEMANTICS.md)
and [Structured Concurrency](../../STRUCTURED_CONCURRENCY_SEMANTICS.md).

---

**Previous:** [← Standard Library](README.md) · **Next:** [ std.io](02-std-io.md)
