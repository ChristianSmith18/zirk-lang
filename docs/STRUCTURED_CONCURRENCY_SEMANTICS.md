# Zirk Structured Concurrency Semantics

> **Surface status:** `concurrent`, `spawn`, `Job<T>`, and `Timer` are the
> delivered structured-concurrency surface. `parallel-cpu-regions`,
> `typed-channels`, and `concurrency-completion` remain separate follow-up
> changes.

## 1. Execution domains

Zirk makes the execution domain explicit. Structured concurrent operations run
on the single-threaded cooperative executor. CPU-parallel regions run on the
worker pool. OS threads exist only for affinity and unavoidable blocking.

The compiler preserves the same safety boundary before and after real
parallelism is available: operations may interleave only at defined safe
points, and safe code cannot create a mutable alias that crosses a concurrent
boundary unsafely.

## 2. Structured lifetime

Every concurrent operation belongs to an owner. Normal scope exit waits for its
owned operations. Failure or cancellation requests cancellation of unfinished
siblings, waits for their cleanup, and only then leaves the scope. Work never
becomes ownerless through an implicit detach.

Long-lived services belong to an application-level supervisor. That supervisor
owns startup failure, cancellation, ordered shutdown, resource cleanup, and
final diagnostics.

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

### Timers

`Timer.sleep(Duration)` suspends the current branch and is a cancellation safe
point. `Timer.after(Duration, callback)` runs the callback once after the
deadline. `Timer.every(Duration, callback)` re-arms after each callback until
cancelled. Negative durations are controlled runtime errors before a wait is
armed.

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

Cancellation is idempotent and is observed only at a defined safe point. A
safe point includes a blocking channel operation, a timer wait, `Timer.sleep`,
or an explicit cancellation check. It does not stop a branch at an arbitrary
instruction.

At a safe point, cancellation throws the compiler-known `CancelledError`.
Ordinary `try` / `catch` / `finally` rules apply, so cleanup still runs. A
timeout cancels the owned operation, waits for its cleanup, then reports
`TimeoutError`.

## 5. Capture, transfer, and sharing

The compiler derives `Transfer` and `Share` from the complete reachable type
graph, mutability, resource ownership, and synchronization contract. They are
not ordinary annotations that user code can forge.

- Values and projections cross by value under the normal projection rules.
- A complete `inmut::strict` reference may be shared.
- An exclusive mutable reference may transfer only when the compiler proves
  the sender cannot continue to use it.
- `clone()` creates an independent graph.
- Resources use their explicit transfer operation.
- An ambiguous shared mutable alias is a compile-time error; diagnostics name
  transfer, strict immutable sharing, cloning, or a channel as the remedy.

A concurrent branch cannot mutate a variable captured from an enclosing scope.

## 6. Suspended-operation reachability

The garbage collector keeps references reachable solely from a live suspended
operation. This includes compiler-managed temporaries at a safe point, queued
channel values, and result slots not yet consumed by their owning operation.

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
