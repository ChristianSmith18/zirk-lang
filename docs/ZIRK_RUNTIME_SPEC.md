# Zirk — Runtime specification

## 1. Principles

The runtime provides automatic memory, structured concurrency, parallelism,
non-blocking I/O, timers, cancellation, resources and failure diagnostics. It
must be small, portable and linkable into standalone binaries. Subsystems are
initialized lazily where possible.

Zirk exposes no global event loop. The runtime may internally use one or more
event reactors.

The exact base-ten `Float` type is a value (a 128-bit coefficient plus a
decimal scale) with no heap allocation and no participation in garbage
collection, the same class as `Duration`. Operations an integer instruction
cannot express — scale alignment, half-to-even rounding, exact formatting,
parsing, and conversion to or from a `Float` — are `extern "C"`
`zirk_rt_decimal_*` helpers; every `Float` and 128-bit integer crosses that
boundary by pointer, and a `Float`/`i128` result returns through a leading
out-pointer. Coefficient overflow, a negative `sqrt`, and division by zero
raise the same controlled failures the rest of the language uses.

## 2. Application lifecycle

Normative order:

```text
validate .zkinit and permissions
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

**Implementation status.** The generated C `main` calls `zirk_rt_init`, then
`zirk_rt_run_main_cli(zirk_main)`, then — as before — checks for an uncaught Zirk
exception, then `zirk_rt_shutdown`. Both `zirk_rt_run_main` and
`zirk_rt_run_main_cli` run `zirk_main` as the
**root task (task 0)** of the single-threaded cooperative executor (`ADR-017`)
and returns only once task 0 and every task it spawned has completed or been
cleaned — so a background task keeps the process alive after `main` returns. The
root task gets an 8 MiB stack (`main` historically ran on the OS main-thread
stack); spawned tasks get 128 KiB. For a program with no `concurrent` / `spawn` yet,
the selected run-main entrypoint is exactly one run of `zirk_main` to completion.

The runtime also has a provisional, runtime-internal detached-job entry point
(`zirk_rt_background_spawn`), separate from ordinary structured spawn. It
admits at most 256 live jobs per executor run, retains each capture block as a
GC root, and records completion/failure with the background supervisor. When
the root task completes, admission closes, detached jobs receive cooperative
cancellation, and the executor allows a fixed five-second grace before
reporting unfinished job IDs to stderr. This is not yet a Zirk language API:
`App.background`, `Concurrent.detach`, configurable grace, services, signal
handling, and managed native-thread integration remain unavailable. The
executor cannot preempt a task that never yields.

The provisional background-spawn ABI distinguishes status `1` (background
quota full) from status `4` (the executor's separate 4,096-task quota full),
so source lowering can report `BackgroundCapacityError` and `TaskCapacityError`
respectively. Status `2` means background admission is draining; status `3`
means an internal duplicate/ID-exhaustion condition. Source-level status
translation is not implemented yet.

The Rust runtime also exposes a bounded `ServiceSupervisor` lifecycle model
(default capacity 64). It tracks `Created`/`Starting`/`Ready`/`Draining`/
`Stopped`, required-service readiness, and reverse actual-start-order shutdown.
Services may declare dependencies on already registered services; startup is
rejected until those dependencies are ready, and the registration rule makes
dependency cycles impossible. This module records state and ordering only: it
does not launch service tasks, retain their managed captures, receive host
signals, or provide the Zirk `application.spawn_service` API.

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

**Implementation status (roadmap Phase 5, steps 1–3).** The delivered scheduler
is a **single-threaded cooperative executor**: one operating-system thread, a
first-in-first-out ready queue, and a monotonic timer heap consulted once per
scheduling turn. It never preempts a running task — a task yields only at a safe
point (a suspending `Job.wait`, channel operation, timer wait, or compiler
safepoint poll in a loop). This is distinct from the separately delivered
CPU worker pool used by `parallel` regions (see §6): the general task scheduler
does not yet use multicore scheduling or work stealing, and there is no I/O
reactor. I/O calls still block, and managed language threads (`Thread.run`)
remain unavailable. The multicore scheduler and reactor described above are
roadmap Phase 5 steps 4–6.

Each managed executor task runs as a **stackful coroutine** (`ADR-017`): it owns a
heap-allocated stack (fixed 128 KiB) with an OS guard page, and suspension performs
a cooperative context switch back to the executor. A plain function call keeps
the ordinary calling convention whether or not the callee awaits internally —
there is no `async fn` and no function-body transformation. The context-switch
seam lives in `crates/zirk-runtime/src/context.rs`; on the supported target
triples (macOS aarch64, Linux x86_64, Linux aarch64, Windows x86_64 — `ADR-004`
addendum) it is provided by `corosensei`, and every other host uses a
thread-backed fallback with identical semantics so the test suite builds
everywhere. Garbage-collection roots are enumerated through one shadow-stack
chain per task, so a suspended task's references stay reachable across a
collection.

## 4. Structured concurrency

The delivered source surface is `concurrent { ... }`, `spawn`, linear
`Job<T>` handles, and `Timer`. The former `task` / `await` / `select` language
surface was removed; runtime task IDs and outcomes are internal scheduler
concepts, not source-level `Task<T>` values.

`concurrent` establishes a scope for branches, and `spawn` creates a dynamic
branch whose result is represented by a must-use `Job<T>`:

```text
concurrent {
    mut job = spawn (): Int32 => compute();
    mut result = job.wait();
}
```

`Job.wait()` suspends the current coroutine until the branch completes;
`Job.cancel()` requests cooperative cancellation and `Job.done` observes
completion. Scope exit joins or cancels and cleans up its owned branches.
Unhandled branch failures propagate through scope exit after sibling cleanup.
For non-void generated task results, the scheduler's one-word result is an
opaque pointer to a managed box. The box stores the exact target LLVM layout
of `T`, and its descriptor traces nested references until `Job.wait()` consumes
the result. The branch thunk roots the box before running user code and fills
it before publishing task completion; inline tuples, enums, and wide scalars
are not reinterpreted as pointers or truncated to one word. Void-like results
use the zero word. This result transport does not expose `Outcome<T>` or
enable the still-gated completion combinators.
`Timer.sleep`, `Timer.after` and `Timer.every` provide the delivered timer
operations. General detachment and source-level aggregation/select combinators
remain unavailable.

Each executor admits at most 4,096 nonterminal tasks and 4,096 active timer
registrations. Generated Zirk calls use status-bearing runtime ABIs and lower
saturation to catchable `TaskCapacityError` or `TimerCapacityError` values; the
ABI does not encode failure as a task ID. `Timer.after/every` reserve task and
timer capacity atomically before their callback becomes runnable. A task permit
is returned after terminal completion and a timer permit after expiry or
disarm; timer-job reservations persist across periodic waits until the job
terminates.

## 5. Cancellation and timeout

Cancellation is cooperative and idempotent. Job cancellation and scope exit
request cancellation; the runtime does not destroy execution at an arbitrary
instruction. Blocking channel/timer operations return at cancellation safe
points, and generated cleanup code observes and delivers cancellation after
cleanup. Loop back-edges poll collector safepoints; those polls do not by
themselves preempt or cancel a running task.

Timeout operators and source-level cancellation shields are not part of the
currently available language surface. Timer durations are represented by the
runtime timer API.

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
associative primitives or safe accumulators. Ordered map-like operations
preserve input order; completion-order variants are explicitly unordered.
Grouping-sensitive reductions use a deterministic variant.

**Delivered pool and admission behavior.** The runtime's separate parallel
worker pool is capped at 64 native workers, including when
`ZIRK_PARALLEL_CORES` requests a higher count. Its pending-job queue is bounded
to `min(4 × configured_workers, 1,024)`; submissions block with backpressure
while the queue is full. When the submitting branch belongs to the executor,
timed waits poll collector safepoints so a worker-triggered collection can progress.
For `parallel_for`, the runtime partitions the index range into at most the
queue-window number of jobs and polls the collector after every iteration, so
large ranges do not allocate one closure per element. `run_region` consumes an
exact-size job iterator directly, and `parallel_map` creates closures on demand
as the bounded queue accepts them. Ordered result slots remain proportional to
the input length; generic callers that construct a `Vec` before calling
`run_region` also retain that caller-owned vector.

`Executor::with_background_shutdown_grace(Duration)` configures the detached-job
shutdown grace for runtime callers; `Executor::new()` uses the five-second
`BACKGROUND_SHUTDOWN_GRACE` default. When that grace expires, the runtime reports unfinished
jobs. Generated CLI executables exit with status 124; embedded hosts select
`zirk_rt_run_main_with_deadline_policy` with policy `0` to wait safely or policy
`1` to exit with status 124. The generic `zirk_rt_run_main` uses policy `0`.
Process-level exit does not free the heap while native callbacks can still
reference it; the operating system reclaims it with the process. The wait policy
continues servicing safepoints and joining outstanding parallel
regions before unpublishing or freeing the shared heap. Thus the grace deadline
does not forcibly terminate workers or authorize releasing their roots or heap
while they may still use them. A worker that never returns can therefore keep
the wait policy, and host-controlled shutdown, waiting indefinitely. This guarantee covers
the parallel pool remains distinct from the managed `Thread.run` worker pool.
`Thread.run` uses at most 64 process-wide workers and 1,024 queued callbacks;
when the queue is full, the caller yields until it can submit. The worker
registers as a collector mutator, and the caller retains its rooted callable,
result, and exception slots until cooperative join. Caller cancellation does
not abandon an accepted callback. Source execution is supported on native
64-bit targets; Wasm and 32-bit pointer ABIs fail closed. Blocking native calls
whose effect and collector transition cannot be proven remain rejected.

The background supervisor retains at most the 1,024 most recent diagnostics;
recording a newer diagnostic evicts the oldest one. This is a bounded runtime
history, not a durable audit log.
The runtime-only error callback runs outside the supervisor state lock and may
be called concurrently when jobs fail on different completion threads. Its
closure must therefore support concurrent calls. A failure reported recursively
from the same callback thread is recorded as suppressed instead of recursively
calling the handler; callback panics are contained and retained as diagnostics.

## 7. Threads

`Thread.run(fn)` is available on supported native 64-bit targets through a
bounded managed worker pool. The collector registers each callback worker and
scans its shadow roots at safepoints. The caller yields to the executor and
joins the worker before its rooted capture/result slots can leave scope. Queue
capacity is 1,024 and the process-wide worker ceiling is 64. Caller
cancellation does not abandon accepted work. Wasm and 32-bit pointer ABIs are
rejected by codegen. Blocking FFI calls remain rejected unless a complete
effect summary and the generated `InNative` transition can both be proven.
The separate `Thread<T>` handle API remains unavailable.

`Thread.run` invokes its callback on a real managed operating-system worker:

```text
mut native_thread = thread "worker" {
    process();
};

native_thread.join();
```

A thread has an optional name, a result, `join` and a cooperative cancellation
request where applicable. It cannot be implicitly abandoned when its scope ends.
Sharing mutable memory requires `sync`, a mutex or atomics.

Legacy blocking APIs run on the calling thread; a managed blocking executor is
not currently available.

There is no `worker` as a language entity. An isolated or dedicated worker is
built from a supervised thread/task and one or more channels.

## 8. Channels, synchronization and atomics

Channels are implemented as listed in the feature status catalog. Scalar
`Atomic<T>` is available for Boolean and signed/unsigned integers through 64
bits when the target provides the matching atomic width. Its payload is stored
inline in the GC-managed object; pointer atomics are unsupported. The public
`Mutex`, `RwLock`, `Semaphore`, `Barrier`, and `Once` APIs remain unavailable.

`Channel<T>` is a typed queue for communication between cooperative branches.
Channel operations inside `parallel` regions are rejected because pool
callbacks cannot suspend; communication with the parallel worker pool or
managed OS threads is not currently exposed:

```text
mut messages: Channel<String> = Channel();
messages.send("ok");
mut message = messages.receive();
```

`receive` suspends efficiently inside a cooperative branch. Try operations return typed
outcomes that distinguish success, closure, and temporary absence/fullness.
Channels support explicit closing,
wake waiters, drain queued values, and apply backpressure on bounded channels.

At concurrent boundaries values and projections copy, strict immutable
references may share, exclusive mutable references may transfer, clones become
independent, and synchronization-aware references may share. `Transfer` and
`Share` are internal compiler-derived properties, not forgeable user traits.

`sync` delimits protected access. The future synchronization API must prevent
holding a mutex across a suspending operation unless that lock explicitly
supports it; the compiler/linter diagnoses this.

`Atomic<T>` exposes `load`, `store`, `exchange`, `compare_exchange` and integer
`fetch_add`/`fetch_sub`. Operations default to `SeqCst`; weaker orderings require
`unsafe`. Statically invalid order combinations are rejected during compilation;
a dynamically invalid order raises `InvalidArgumentError` before the atomic is
accessed. No pointer atomic operation is available. This API protects individual
atomic operations and does not make arbitrary shared state race-free. The public
`RwLock<T>`, `Semaphore`, `Barrier`, and `Once<T>` APIs remain unavailable.

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
- `Weak<T>` does not keep a referent alive and upgrades through `Option<T>`;
- dependent native/resource/internal views cannot escape their owner;
- deep cloning preserves internal sharing and cycles in a new identity graph.

`Object` values and class instances are identity-bearing managed references.
Dynamic properties carry runtime type tags and descriptor-visible references so
the collector traces nested objects, collections and callables. Contract
conformance seals the key set on the shared object allocation; the seal is
alias-visible and does not alter object identity. Tuples and enums retain their
value layouts; there is no record-only inline representation.

There are no general-purpose destructors whose timing is observable. Memory
release is not used to manage files, sockets, locks or processes.

Raw `Pointer<T>` may be null and requires unsafe for construction,
dereference, arithmetic, casts, and volatile access. Bounded
`NativeSlice<T>`/`NativeSliceMut<T>` views carry checked extent and lifetime.

An ordinary unsafe block journals managed and validated-range writes. It commits
on success and rolls back on controlled failure before commit. External I/O,
unknown FFI, volatile/device effects, manual release, concurrent publication,
and unproven raw writes require `commit {}`. Reversible transactions cannot
suspend or spawn work. Arbitrary corruption and true undefined behavior are not
recoverable guarantees. `MEMORY_AND_UNSAFE_SEMANTICS.md` is normative.

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
