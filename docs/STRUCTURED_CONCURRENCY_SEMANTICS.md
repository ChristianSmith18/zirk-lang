# Zirk Structured Concurrency Semantics

> **Surface status:** `concurrent`, `spawn`, `Job<T>`, `Timer`, `parallel`, and
> typed `Channel<T>` (section 8) are delivered. `concurrency-completion` is an
> active follow-up; its runtime foundations do not make the planned thread,
> synchronization, or aggregation APIs available in Zirk source.

## 1. Execution domains

Zirk distinguishes the delivered single-threaded cooperative executor from the
CPU worker pool used by `parallel`. The language does not currently expose a
managed OS-thread API. Synchronous I/O may block the cooperative executor; a
general asynchronous I/O reactor is not delivered.

Concurrency checks apply at implemented boundaries. For example, the compiler
rejects mutable reference captures in `parallel`; this does not establish a
general data-race guarantee for every shared value or global.

## 2. Structured lifetime

Delivered `concurrent` scopes own their structured branches. Normal scope exit
joins owned work; failure or cancellation requests cancellation and cleanup of
unfinished branches before leaving the scope. There is no general source-level
detach API.

An application-level service supervisor is a design contract, not a delivered
Zirk API. The current runtime contains a provisional internal background-job
supervisor; it is not integrated with language-level services or managed
threads. See the runtime specification for its admission and shutdown limits.

### `concurrent` and `spawn`

`concurrent { ... }` opens a lexical structured scope. A direct `inmut` or
`mut` declaration in the block is a branch whose binding is available in the
enclosing scope only after the block closes. Independent bindings may run
together; a binding that reads a sibling is ordered after that sibling. Cycles
and reads that occur before a sibling is available are compile-time errors.

`spawn expr` and `spawn { ... }` create dynamic, joinable branches. They are
valid only in a `concurrent` block or in `main`'s implicit root scope. Their
result is `Job<T>`; `job.wait()` consumes the handle and returns `T`,
`job.cancel()` requests cancellation, and `job.done` reports terminal state.
An unconsumed job is diagnosed unless explicitly discharged with `_ = job`.
Each executor admits at most 4,096 nonterminal tasks. A rejected `spawn`
raises `TaskCapacityError`; completed outcomes do not consume active-task
capacity while they remain joinable.

### Timers

`Timer.sleep(Duration)` suspends the current branch and is a cancellation safe
point. `Timer.after(Duration, callback)` runs the callback once after the
deadline. `Timer.every(Duration, callback)` re-arms after each callback until
cancelled. Negative durations are controlled runtime errors before a wait is
armed. Each executor admits at most 4,096 active timer registrations. A
rejected wait or timer registration raises `TimerCapacityError`. `after` and
`every` reserve both task and timer capacity before publishing the callback;
their timer reservation remains available across periodic waits and is released
when the callback task reaches a terminal state.

`after` and `every` return `Job<T>` but are ambient: they belong to the nearest
lexical `concurrent` scope (or `main`) and scope close cancels them instead of
waiting for their natural completion. This prevents a periodic timer from
keeping a finite scope alive. A program that explicitly waits a timer job owns
that wait in the ordinary way.

## 3. Failure propagation

An unhandled throwable in a concurrent branch is a branch failure. Its owner
must:

1. preserve it as the primary failure;
2. cancel active sibling branches;
3. wait for their cleanup;
4. attach cleanup or sibling failures as suppressed; and
5. propagate the primary failure after cleanup.

A returned `Result.Error` remains an ordinary successful value; it is never an
unhandled branch failure merely because it carries an error value.

## 4. Cooperative cancellation

Cancellation requests are idempotent and cooperative. The executor wakes a
branch parked in a cancellation-aware wait; generated code checks cancellation
at branch entry and at selected call/cleanup boundaries. Cancellation does not
preempt arbitrary instructions, and the compiler does not insert a cancellation
poll into every loop back-edge. A tight loop or synchronous native call that
does not reach a checked boundary can therefore delay cancellation.

When delivered, cancellation takes the runtime's controlled cancellation path;
it is not an ordinary catchable `CancelledError`. Generated cleanup runs under
a cancellation shield, and cleanup failures do not replace the cancellation
outcome. `Job.cancel()` requests cancellation; it does not itself wait for the
job to finish. `Concurrent.of(...).first()` and `.settled()` support a
`.within(Duration)` deadline that cancels and joins their branches and raises
`TimeoutError`; caller cancellation remains cancellation. Deadline timers are
compiler-owned and cleaned up with the pipeline. `Concurrent.each` and
`Concurrent.each_settled` accept `Array<T>` and `List<T>` with at most 256
elements. Their input must be held through an `inmut::strict` binding and the
complete element graph must satisfy `Share`; the default concurrency is 32,
and an explicit limit must be 1 through 256. Both preserve input order and
reject invalid size or limit with `InvalidArgumentError` before starting a
callback. `each` cancels and joins admitted siblings after the first failure;
`each_settled` returns one typed `Outcome<T>` per callback. Caller cancellation
cancels and joins admitted callbacks and returns no partial list. Within
`Concurrent.of(...).settled()`, throwing `CancelledError` reports cancellation
of that member; its `finally` cleanup completes before collection, and a
different cleanup error is reported as `Rejected`. These collection
combinators currently require the native scheduler; the browser interpreter
reports its managed-scope/outcome capability exclusion.

## 5. Capture, transfer, and sharing

The checker derives conservative `Transfer` and `Share` properties for the
represented type graph and enforces them at supported boundaries, including
spawn, channels, `parallel`, and `Parallel.each`. This is not a complete gate
for every proposed concurrency API: managed `Thread.run`, scoped
`Mutex<T>.with`, and scalar `Atomic<T>` are available, while general
`Thread<T>` handles and other synchronization APIs are not; `Concurrent.of(...).first()` / `.settled()` are lowered only for homogeneous direct thunk pipelines. Derivation covers concrete scalar and aggregate/container types,
tuples, unions, and checked classes/enums with generic substitution. Unknown
or open contracts, function values without capture provenance, task handles,
pointers, weak/native views, and opaque native handles conservatively fail the
proof; this is not a universal ownership guarantee for arbitrary native code.

- Values and projections cross by value under the normal projection rules.
- A complete `inmut::strict` reference may be shared.
- An exclusive mutable reference may transfer only when the compiler proves
  the sender cannot continue to use it.
- `clone()` creates an independent graph.
- Resources use their explicit transfer operation.
- An ambiguous shared mutable alias is rejected at the boundaries where the
  checker currently enforces these rules; diagnostics and suggested remedies
  are not a universal API contract.

A concurrent branch cannot mutate a variable captured from an enclosing scope.

## 6. Suspended-operation reachability

The collector traces generated shadow-stack root slots for live executor tasks,
including tasks while suspended, and retains task results until their join
result is consumed. Non-void generated results use a managed box with a
compiler-emitted descriptor that traces references nested inside the exact
result layout. The task control block roots the box until consumption. The
collector also traces managed references in queued channel values using the
element layout supplied by code generation. These guarantees apply to roots
the compiler/runtime registers; arbitrary Rust locals, unregistered native
stacks, and foreign pointers are not scanned as roots. Native mutators
that join the published heap must register with the collector and participate
in its stop-the-world protocol; the internal managed-blocking pool is not
connected to that protocol and accepts only Rust-owned closures.

The runtime represents an internal scheduler task as a stackful coroutine with
its own shadow-stack roots. That scheduler term is not a language-level handle
type and does not reintroduce the removed `Task<T>` surface.

## 7. Runtime and implementation boundary

The Phase 5 runtime provides the cooperative executor, branch control blocks,
stackful suspend/resume driver, timer service, per-branch GC roots, and the
executor lifecycle around `main`. It does not define language syntax by itself.

`concurrent-blocks-and-timers` supplies structured blocks and dynamic branches;
`parallel-cpu-regions` supplies CPU regions and their safe points;
`typed-channels` supplies typed communication; and
`concurrency-completion` supplies the remaining policy and outcome APIs.

The current effect analysis is narrower than a general effect system. It
propagates `may_suspend`, `may_block`, and `may_spawn` through checked direct
calls. Known language seeds currently mark suspension and spawning; unresolved
callable and `extern` effects are marked incomplete and conservatively treated
as possibly suspending, blocking, and spawning. Those summaries guard calls
inside `parallel` and reject incomplete or potentially effectful cleanup lambdas
for `Concurrent.protect`. The API lowers a zero-argument cleanup lambda under
a nesting-safe cancellation shield, then checks cancellation before dispatching
a pending exception. Cleanup must terminate: the checker does not prove
termination or a time bound. The
checker does not classify every synchronous I/O/native call or guarantee that
arbitrary code cannot block the cooperative executor. Executable `Concurrent.of`
pipelines remain limited to supported direct homogeneous thunk terminals; the
collection combinators are not lowered.

The language exposes managed `Atomic<T>` for booleans and signed/unsigned
integers through 64 bits when the target advertises that atomic width. Its
scalar payload is inline in the GC-managed object; pointer atomics are
unsupported. Operations default to sequential consistency, weaker orderings
require `unsafe`, and invalid dynamic order combinations raise
`InvalidArgumentError`. There is no lock-free promise. The language still has
no managed `Thread.run` or mutex, rwlock, semaphore, barrier, or once API.
`parallel` uses OS worker
threads; a separate bounded blocking pool exists only as an unregistered,
private runtime foundation for Rust closures.

When `main` completes, the executor closes background-job admission, requests
cancellation of its tracked background tasks, and waits up to its configured
shutdown grace. Expiry records unfinished jobs; it cannot forcibly stop a
non-cooperative task. The executor still waits for active `parallel` regions
before unpublishing/freeing the managed heap, so the grace period is not a hard
process-shutdown bound. The service supervisor is lifecycle bookkeeping only;
it is not wired to launch Zirk services or coordinate this shutdown.

## 8. Typed channels

`Channel<T>` is a shareable endpoint. `Channel<T>()` creates a growable channel
with a runtime defense limit; `Channel<T>(capacity)` creates a bounded channel,
including rendezvous semantics at capacity zero. `Channel.unbounded<T>(limit)`
is the explicit growable form. `send` and `receive` are cooperative safe
points, while `try_send` and `try_receive` never suspend and return the typed
`SendOutcome` / `ReceiveOutcome<T>` enums. Closing is idempotent: queued values
remain receivable, then `receive` returns `None`; sends after closure fail and
the non-blocking operations return their `Closed` outcome. `length`,
`capacity`, and `is_closed` are runtime queries. `drain()` is specified to
consume values in FIFO order through closure.

`select` is removed from the language. Multi-source waiting uses
`Concurrent.of(...).first()` from the completion change instead.

## 9. `parallel` CPU regions

`parallel { ... }` opens a CPU-bound region, run on the worker pool
(`crates/zirk-runtime/src/pool.rs`) rather than the cooperative executor. A
`for x in coll { ... }` that fills the whole region body, over an
`Array<T>`/`List<T>`, and whose body has no `return`/`break`/`continue`/
`throw` and reassigns no captured name, is split into one chunk per element
and dispatched across the pool. A `parallel` block used where a value is
expected types as its final expression. Every other shape (a `for` over a
`Range`, a disqualified loop body, anything other than one bare `for` filling
the region) runs the body sequentially, inline, on the calling thread — still
correct, just without the parallel speedup.

An optional `;`-separated header sets `cores` before the block: `cores: N`
(`N > 0`) uses exactly `N` worker threads; `cores: -N` uses the detected core
count minus `N` (a runtime error if that is not positive); `cores: A..=B`
lets the runtime pick within that range; absent, the region uses every
detected core. `cores: 0` is a compile-time error. `chunk` is grammar-reserved
for a future chunk-size option; not implemented yet.

A `parallel` region performs no I/O and no suspension: `Timer.*`, `spawn`,
`concurrent { }`, and `println` are all rejected inside one, because pool
threads have no safe points for them. The capture rules of section 5 apply at
the region boundary too, with one narrower addition: a captured *scalar* (not
just a reference type) that the loop body reassigns disqualifies the region
from work-splitting and falls back to the sequential lowering above, because
each chunk would otherwise mutate its own private copy and the reassignment
would go nowhere.

`collection.parallel` keeps the collection's ordinary static type, but routes
supported terminals (`map`, `filter`, `for_each`, associative `reduce`, and
`sum`) through the worker pool. `Parallel.each(coll, fn)` uses the same ordered
result-buffer path and returns `List<R>` in input order. `reduce_ordered`
remains sequential by definition. Unsupported terminals retain their normal
sequential lowering rather than being presented as parallel work.
