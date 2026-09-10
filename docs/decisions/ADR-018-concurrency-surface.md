# ADR-018: Structured concurrency surface

## Status

Accepted

## Context

The Phase 5 executor provides stackful branch suspension, a timer service, and
per-branch GC roots. The former `task` / `await` language surface was removed;
the replacement must preserve one ordinary function ABI and make branch
ownership explicit.

## Decision

Zirk uses `concurrent { ... }` as its structured-concurrency statement and
`spawn` for dynamic branches. A `concurrent` block joins every owned spawned
branch before it closes. Direct `inmut` and `mut` bindings in the block are
branch results hoisted into the enclosing scope, but cannot be read before the
block closes except by a dependent branch.

The compiler builds Rule B's dependency DAG over those direct bindings: a
binding that reads another binding starts only after that dependency completes;
independent bindings run concurrently. A cycle is a compile-time error.

`spawn expr` and `spawn { ... }` are permitted only in a `concurrent` scope or
the implicit root scope of `main`. They return `Job<T>` when bound. `job.wait()`
consumes the result once, `job.cancel()` requests cooperative cancellation, and
`job.done` reports completion. The surrounding scope still owns and joins the
branch.

`Timer.sleep` suspends the current branch without changing the function's type
or ABI. `Timer.after` and `Timer.every` create ambient timer jobs owned by the
nearest lexical `concurrent` scope. On scope close, ordinary `spawn` branches
are awaited; ambient timers are cancelled unless their handle is explicitly
waited. This prevents a periodic timer from accidentally making its owner
infinite while retaining explicit control for callers that need it.

Cancellation is cooperative and observed only at safe points. A failing branch
cancels siblings, waits for cleanup, and propagates its primary failure with
subsequent failures suppressed. Returned `Result.Error` values remain ordinary
branch results.

## Consequences

There is no `async` function coloring and no alternate calling convention.
Compiler, runtime, IR, and codegen work identify branch birth and scope exit,
while ordinary functions can suspend internally. `parallel`, channels, timeout
combinators, detachment, and synchronizers remain separate Phase 5 changes.
