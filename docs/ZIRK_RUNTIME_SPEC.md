# Zirk — Runtime specification

## 1. Principles

The runtime provides automatic memory, structured concurrency, parallelism,
non-blocking I/O, timers, cancellation, resources and failure diagnostics. It
must be small, portable and linkable into standalone binaries. Subsystems are
initialized lazily where possible.

Zirk exposes no global event loop. The runtime may internally use one or more
event reactors.

## 2. Application lifecycle

Normative order:

```text
validate init.zrk and permissions
    ↓
load minimal runtime
    ↓
initialize globals in a deterministic order
    ↓
invoke main()
    ↓
run structured concurrency scopes
    ↓
ordered shutdown of resources and managed threads
    ↓
flush streams and terminate with an exit code
```

`main` is the only executable symbol of the `project.entry` file:

```text
fn main(): Void { ... }
```

It may also return a code or a `Result` defined by the entrypoint contract. A
failure during globals prevents `main` from running and produces a deterministic
diagnostic.

Normal termination happens when `main` ends and all of its root scopes have
concluded. Orphan work is never silently awaited.

## 3. Scheduler and I/O reactor

The scheduler runs tasks on a multicore pool sized from hardware and safe
configuration. It may use local queues, work stealing, internal priorities and
affinity, but it does not guarantee that a task stays on one thread.

The reactor integrates I/O, timers and signals using native mechanisms such as
IOCP, `epoll` or `kqueue`. A task waiting on I/O suspends without needlessly
reserving a thread; when the operation completes it returns to the runnable
queue.

The scheduler must avoid starvation, bound queue growth and apply backpressure
where the contract permits.

## 4. Tasks and await

`task` creates managed concurrent work:

```text
mut operation = task {
    load_data();
};

mut result = await operation;
```

`await` suspends the current task, not an operating-system thread. There is no
`async fn`: the type of a function describes its logical value and `task` makes
concurrent execution explicit.

Tasks are typed, propagate result/error and belong to a scope. On leaving the
scope, their children must have finished or must receive cooperative
cancellation and be awaited. No task may be implicitly orphaned.

A future detached operation, if introduced, must be explicit and transfer
ownership to a root supervisor; it is not part of the initial contract.

## 5. Cancellation and timeout

Cancellation is cooperative, idempotent and observable at safe points: `await`,
I/O, channels, timers and checks in long-running loops. `task.cancel()` requests
cancellation; it does not destroy execution at an arbitrary instruction.

Cancellation propagates from parent to children. The task performs resource
cleanup and finishes with a typed recoverable cancellation exception, unless its
API explicitly turns it into a `Result`.

```text
await load_data() timeout 5s;
```

A timeout requests cancellation and produces a recoverable timeout exception.
Durations admit `ms`, `s`, `m` and `h` and are represented internally with
sufficient precision, even if they may be normalized.

## 6. Parallel

`parallel` expresses potentially simultaneous CPU work:

```text
parallel {
    process_a();
    process_b();
}
```

`parallel for` distributes independent iterations across the pool:

```text
mut squares = parallel for value in 0..1000 {
    value * value
};
```

The runtime decides partitioning and the number of internal workers. Results
preserve an order defined by the operation contract, not by physical completion
order. If an iteration returns a `Result`, the first relevant error is kept,
remaining work is cancelled cooperatively and its shutdown is awaited.

Unsafe mutable captures are a compile error. Reductions must use explicit
primitives or safe accumulators.

## 7. Threads

`thread` creates a real operating-system thread:

```text
mut native_thread = thread "worker" {
    process();
};

native_thread.join();
```

A thread has an optional name, a result, `join` and a cooperative cancellation
request where applicable. It cannot be implicitly abandoned when its scope ends.
Sharing mutable memory requires `sync`, a mutex or atomics.

There is no `worker` as a language entity. An isolated or dedicated worker is
built from a supervised thread/task and one or more channels.

## 8. Channels, synchronization and atomics

`Channel<T>` is a typed, thread-safe queue between tasks, parallel work and
threads:

```text
mut messages: Channel<String> = Channel();
messages.send("ok");
mut message = await messages.receive();
```

`receive` suspends efficiently inside a task. `try_receive` returns `Option<T>`
or the optional type defined by the stdlib. Channels must support explicit
closing, distinguish closure from temporary absence and apply backpressure on
bounded channels.

`sync` delimits protected access. Mutexes must not be held across `await` except
for a type expressly designed for it; the compiler/linter diagnoses this.

`Atomic<T>` exists only for supported types and operations. It exposes
load/store, exchange, compare-exchange and numeric operations such as increment.
The default memory ordering must be safe; weaker orderings are explicit and
advanced.

## 9. Memory

Management is automatic. Stack/heap, escape analysis, regions, moves, RC or GC
are internal details that may be combined. The representation never alters
equality, identity or observable lifetime.

Requirements:

- no use-after-free in safe code;
- no double-free;
- cycles and concurrency must be released correctly;
- pauses and consumption must be measured;
- value types may be stored inline;
- objects with identity keep a stable identity even if physically moved.

There are no general-purpose destructors whose timing is observable. Memory
release is not used to manage files, sockets, locks or processes.

## 10. Resources

External resources implement `Resource<E>` and are managed with `match with`.
Closing happens exactly once when leaving the block through success, error,
exception, return or cancellation. Errors while opening are handled before
acquiring the resource; errors while closing follow the typed contract of the
resource and must not silently hide a primary error.

There is no `defer` in 1.x. The compiler prevents a managed resource or a
dependent reference from escaping the scope.

Grouped acquisitions proceed left-to-right and unwind right-to-left. A body
`Result.Error` and close error compose as `ResourceFailure.BodyAndClose`; a
close error during exception propagation is suppressed on the primary
throwable. Transfer invalidates the old responsibility. Resources are not
ordinarily cloneable, dependent resources cannot outlive parents, and dynamic
closed/transferred misuse is a typed runtime failure.

## 11. Signals and ordered shutdown

The stdlib translates supported signals (`SIGINT`, `SIGTERM` or equivalents)
into shutdown events. The root supervisor:

1. marks the shutdown state;
2. requests cancellation of root scopes;
3. wakes cancellable waits;
4. closes resources in reverse acquisition order;
5. joins managed threads;
6. flushes `stdout`/`stderr` within a bound;
7. terminates with an exit code.

A second signal or an exhausted bound may force a controlled exit. `fatalError`
attempts to emit a diagnostic and perform only the cleanup that is safe; it does
not promise to continue normal execution.

## 12. Security and permissions

The runtime enforces declared permissions for filesystem, network, processes,
environment and other capabilities. A library declares requirements; the
application grants the final set. The absence of a permission produces a clear
error, not an automatic grant.

The effective grant is the intersection of signed developer approval and the
application declaration. Approval is external and bound to project name,
canonical location, user/device, scope/phase and exact requester
versions/integrity/paths. Filesystem targets are canonicalized with symlink
escape protection; network redirects and DNS results remain in origin scope;
process arguments and shell authority are checked separately; secrets redact.
Runtime never prompts or self-edits a manifest.

Explicit throwables preserve identity, cause and suppressed failures. Traces
materialize lazily, reconstruct logical task/generator frames where metadata
permits, and never capture locals or secrets by default.

`unsafe` does not bypass permissions or operating-system validations.

## 13. Observability

Stack traces include functions, tasks, threads, awaits and decorator expansions
where metadata exists. The debugger may enumerate pending tasks, timers,
channels and threads. Internal metrics are not part of the semantics except
through explicit APIs.
