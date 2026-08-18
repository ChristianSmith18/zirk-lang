# Zirk Structured Concurrency Semantics

This document is the normative source of truth for tasks, awaiting, structured
scopes, failure, cancellation, aggregation, selection, channels, parallelism,
threads, synchronization, atomics, transfer, and data-race safety.

## 1. Choosing an execution primitive

| Primitive | Intent | Scheduling | Lifetime owner |
|---|---|---|---|
| `task` | Concurrent logical work, especially waiting | Runtime scheduler | Lexical task scope |
| `parallel` | Finite CPU work across cores | Runtime CPU pool | Parallel expression/scope |
| `thread` | Native affinity or unavoidable blocking isolation | Operating system | Lexical thread scope |
| `Channel<T>` | Typed transfer and coordination | Task/thread aware | All retained endpoints |

There is no `async fn`. A function describes its logical input/output contract;
`task` explicitly decides to execute a call concurrently. There is no standalone
`worker` primitive: a worker is composed from supervised tasks or threads and
channels.

## 2. Tasks and types

`task` returns `Task<T>`:

```zirk
mut user: Task<Result<User, LoadError>> = task load_user(42);
mut result: Result<User, LoadError> = await user;
```

Block and callable forms are equivalent:

```zirk
mut first = task {
    return load_user(1);
};

mut second = task load_user(2);
```

`await` suspends the current task, not an OS thread, and produces exactly `T`.
It does not introduce implicit `Result`, nullability, or exception wrapping.
Task creation starts the child immediately. Await consumes its `Task<T>` result
exactly once; a second await is a compile-time use-after-consume error. Multiple
observers use explicit watch/broadcast/channel contracts or a deliberately
shared deeply immutable value.

## 3. Structured scopes

Every executing function belongs to a task scope. `task scope` creates an
explicit nested supervisor:

```zirk
mut dashboard = task scope {
    mut users = task load_users();
    mut roles = task load_roles();
    return combine(await users, await roles);
};
```

A scope cannot finish while a child is silently running. Normal exit waits for
unfinished children. Exceptional exit or cancellation requests cancellation,
waits for child cleanup, and only then propagates.

Ignoring a `Task<T>` result is diagnosed under the same explicit-ignore policy
as other must-use results. `_ = task_handle` acknowledges that only completion
and structured cleanup matter; it does not detach the task.

`cancel()` is idempotent and accepts an optional typed reason whose default is
`CancellationReason.Cancelled`. Task names are diagnostic metadata. Scheduling
priority and fairness are runtime-managed rather than user-controlled knobs.

## 4. Failure propagation

An unhandled exception fails its task. Within an ordinary task scope, the first
unhandled child failure:

1. becomes the primary failure;
2. requests cancellation of active siblings;
3. waits for their cleanup;
4. attaches additional failures as suppressed;
5. propagates to the parent.

A returned `Result.Error` is not task rejection. It is an ordinary fulfilled
value that must be handled according to the `Result` contract.

## 5. Long-lived services

General `detach()` is not part of Zirk 1.x. Work that must outlive a request or
local scope is transferred to the application root supervisor:

```zirk
application.spawn_service(fn() {
    return serve_metrics();
});
```

The supervisor owns startup failure, cancellation, ordered shutdown, resource
cleanup, and final diagnostics. Work never becomes ownerless.

## 6. Cancellation

Cancellation is cooperative and idempotent:

```zirk
operation.cancel();
mut value = await operation;
```

Safe points include `await`, task-aware I/O, channel operations, timers, and
explicit checks in long-running loops. Cancellation propagates from parent to
children and throws the compiler-known `CancelledError` unless an API explicitly
converts it to a `Result`.

Cancellation never destroys a task at an arbitrary instruction. The task gets a
chance to close resources and run ordinary cleanup.

## 7. Cancellation shields

A small non-interruptible cleanup section uses:

```zirk
cancellation shield {
    await persist_commit();
}
```

Cancellation remains pending and is delivered immediately after the shield.
Shields must be bounded; implementations should diagnose shields that can wait
indefinitely. Shielding does not suppress unrelated exceptions.

## 8. Timeouts

```zirk
mut value = await operation timeout 5s;
```

At expiry the runtime requests cancellation, waits for cleanup, and throws
`TimeoutError`. The timed operation never continues as an accidental background
task. A timeout duration must be non-negative.

## 9. Aggregating tasks

### `Task.all`

```zirk
mut users: List<User> = await Task.all(tasks);
```

Preserves input order. The first unhandled exception cancels unfinished tasks,
awaits cleanup, and propagates with secondary failures suppressed.

### `Task.first`

```zirk
mut fastest: Response = await Task.first(requests);
```

Returns the first completed task—success or unhandled failure according to its
outcome—and cancels/cleans the remainder.

### `Task.settled`

```zirk
mut settlements: List<TaskSettlement<User>> =
    await Task.settled(tasks);
```

Allows every input to finish and preserves input order:

```zirk
enum TaskSettlement<T> {
    Fulfilled(T),
    Rejected(Throwable),
    Cancelled(CancelledError),
}
```

It never cancels siblings merely because one rejects. A
`Task<Result<User, LoadError>>` returning `Error(problem)` becomes
`Fulfilled(Error(problem))`; only an unhandled throwable becomes `Rejected`.

## 10. Selection

`select` waits until one operation is ready and executes exactly one branch:

```zirk
select {
    message = await messages.receive() => process(message),
    result = await operation => finish(result),
    after 5s => report_timeout(),
    cancelled => cleanup(),
}
```

Eligible guards are task completion, channel send/receive, timers, and the
current cancellation signal. A nonblocking form adds `default`:

```zirk
select {
    message = await messages.receive() => process(message),
    default => continue_other_work(),
}
```

Rules:

- without `default`, selection suspends without blocking a thread;
- with `default`, that branch runs only when no guarded operation is ready;
- multiple ready branches are chosen fairly, not permanently by source order;
- losing operations remain alive and are not implicitly cancelled;
- a branch may explicitly cancel work it no longer needs;
- channel closure is a ready outcome, not an infinite wait;
- branch values follow ordinary copy/transfer rules;
- selected failures propagate normally unless the branch handles them.

## 11. Channels

```zirk
mut events = Channel<Event>();       // unbounded
mut jobs = Channel<Job>(capacity: 8); // bounded
```

Core API:

```zirk
channel.send(value)
channel.receive()
channel.try_send(value)
channel.try_receive()
channel.close()
channel.is_closed
channel.capacity
channel.length
```

Suspendible send/receive cooperate with the scheduler. A full bounded channel
applies backpressure to senders. Try operations never suspend and use typed
outcomes that distinguish success, temporary absence/fullness, and closure.
Closing wakes suspended operations. Values already queued remain receivable;
after the queue drains, receive reports closure.

## 12. Transfer and sharing

The compiler derives two internal properties that users cannot forge:

- `Transfer`: a value may cross to another concurrent execution context.
- `Share`: the same referent may be accessed concurrently.

They do not appear in normal `Fn` annotations.

Rules at a task, channel, parallel, or thread boundary:

- value types and projections copy;
- a complete `inmut::strict` reference may share;
- an exclusive mutable complete reference may transfer, after which the sender
  cannot use it until ownership returns through a structured result/channel;
- `clone()` sends an independent graph;
- synchronization-aware objects may share according to their contract;
- resources transfer only through their explicit resource-transfer operation;
- pointers, locks, task handles, and dependent views cross only when their
  specialized contracts permit it.

## 13. Captures

Task and parallel captures apply the same model:

- values are snapshots at creation;
- projected reads are independent values;
- strict immutable complete references are shared;
- exclusive mutable references are transferred when statically safe;
- synchronized references share;
- ambiguous mutable aliasing is a compile-time error.

```zirk
mut users = [User(name: "Ada")];
mut selected = task { users[0] }; // independent projected value
```

The parent retains `users`; changing the task's returned `User` does not change
the list element.

## 14. Parallel CPU work

`parallel` is for finite CPU-bound work:

```zirk
mut hashes = parallel files.map(hash_file_contents);

parallel for item in items {
    process(item);
}
```

Ordered map-like operations preserve input order regardless of physical
completion order. An explicitly named unordered variant may expose completion
order. The compiler rejects unmanaged blocking I/O, unsafe mutable captures,
unsynchronized shared mutation, and algorithms whose required ordering is not
represented.

## 15. Parallel reductions

```zirk
mut total = parallel numbers.reduce(0, (a, b) => a + b);
```

The combiner must be associative because the runtime may regroup inputs. A
floating reduction can therefore differ slightly from sequential left-to-right
rounding. Use an explicit deterministic reduction when stable grouping is part
of the required result. Mutable accumulation requires a safe reduction
primitive, not a captured shared variable.

## 16. Threads and blocking adapters

`thread` creates scoped operating-system execution:

```zirk
mut native = thread "native-worker" {
    return native_loop();
};

mut result = native.join();
```

Use it for native affinity, unavoidable blocking APIs, stack configuration, or
runtime isolation—not ordinary asynchronous I/O. Scope exit joins or cancels and
joins the thread; it cannot be abandoned.

Legacy blocking work from a task uses a separate pool:

```zirk
mut result = await task.blocking(fn() {
    return legacy_api();
});
```

## 17. Synchronization

Prefer transfer, strict immutable sharing, or channels. Shared mutable state
uses structured synchronization:

```zirk
counter.with(value => {
    value.count += 1;
});
```

`Mutex<T>` scopes access so a guard or writable view cannot escape. An ordinary
mutex cannot be held across `await`; compilation fails because suspension could
deadlock the scheduler or violate ordering. Library synchronizers include:

- `RwLock<T>` for many readers or one writer;
- `Semaphore` for bounded permits;
- `Barrier` for a known participant set;
- `Once<T>` for one-time initialization.

These are library types, not new syntax.

## 18. Atomics

`Atomic<T>` supports only documented boolean, integer, and low-level pointer
forms and operations such as load, store, exchange, compare-exchange, and
selected numeric updates. Atomics protect individual operations, not multi-step
invariants.

Sequential consistency is the safe default. Weaker ordering is an advanced
unsafe operation:

```zirk
unsafe {
    mut observed = counter.load(order: AtomicOrder.relaxed);
}
```

The unsafe transaction rule does not make relaxed ordering correct; the
programmer must still prove the ordering contract.

## 19. Data-race guarantee

Safe Zirk rejects unsynchronized concurrent access when at least one access
mutates shared state. A successful build does not rely on favorable timing to
avoid a race. This guarantee does not imply deterministic completion order:
programs use await order, channels, select, locks, or ordered parallel APIs when
observable ordering matters.

## 20. Unsafe transaction interaction

A reversible unsafe transaction cannot await, spawn, send tentative state,
share it with a thread, or perform an external effect. It must either finish and
commit automatically or enter an explicit irreversible `commit` region first.
See [MEMORY_AND_UNSAFE_SEMANTICS.md](./MEMORY_AND_UNSAFE_SEMANTICS.md).

## 21. Implementation checklist

A conforming implementation must:

- type `task` and `await` without hidden wrappers;
- prevent orphan work and clean children before propagation;
- distinguish `Result.Error` from task rejection;
- implement aggregation policies and ordered settlements;
- select fairly and preserve losing operations;
- apply bounded-channel backpressure and explicit closure;
- diagnose unsafe alias crossing with actionable alternatives;
- preserve ordered parallel results and constrain reductions;
- isolate blocking calls from task scheduler threads;
- reject mutex guards across await and weak atomics outside unsafe;
- reject every safe-code data race within the supported analysis model.

**Related:** [Core Language Semantics](./CORE_LANGUAGE_SEMANTICS.md) ·
[Memory and Unsafe Semantics](./MEMORY_AND_UNSAFE_SEMANTICS.md) ·
[Runtime Specification](./ZIRK_RUNTIME_SPEC.md)
