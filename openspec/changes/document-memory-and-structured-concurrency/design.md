## Context

The current documentation states valuable safety goals but leaves essential
mechanics underspecified. In particular, it does not say which unsafe effects
can roll back, how irreversible effects become visible, how weak/dependent
references behave, or how task failure, aggregation, selection, transfer, and
shared mutation compose. Compiler, runtime, standard-library, documentation,
and phase-planning work all depend on one consistent contract.

The design must retain Zirk's approachable public model: automatic memory,
ordinary reference semantics, no lifetime annotations, no `async fn`, and
structured constructs that make risky or concurrent behavior visible.

## Goals / Non-Goals

**Goals:**

- Make safe-code lifetime and data-race guarantees implementable and testable.
- Define a complete native-memory boundary without exposing internal memory
  management as source-language semantics.
- Make recoverable unsafe mutation atomic where technically possible, while
  identifying effects that cannot honestly be undone.
- Define task lifetime, failure, cancellation, aggregation, selection,
  transfer, parallelism, and synchronization precisely.
- Give every normative rule one canonical owner and align derived documents.

**Non-Goals:**

- Mandating one garbage collector, allocator, scheduler, or threading backend.
- Adding Rust-style ownership, borrow, move, or lifetime syntax.
- Claiming recovery after arbitrary memory corruption or process termination.
- Making external I/O, native libraries, or device effects magically reversible.
- Adding `async fn`, a free-floating detached task, or a standalone worker
  primitive.

## Decisions

### Public memory semantics are strategy-neutral

Zirk specifies reachability, identity, aliasing, projection copies, cloning,
strict immutability, dependent lifetimes, and reclamation of cycles. The runtime
may combine tracing collection, generations, regions, escape analysis, moves,
or reference counting as long as those choices are unobservable. This avoids
locking the language to an implementation while giving programs stable rules.

### Resources, not destructors, own deterministic cleanup

Managed objects have no observable general-purpose finalizer. Deterministic
cleanup remains the role of `Resource<E>` and `match with`. This avoids GC
timing becoming program behavior and preserves the already accepted failure
composition rules.

### Weak and dependent references are explicit capabilities

`Weak<T>` never keeps a referent alive and must be upgraded to `Option<T>`.
Native slices, pinned views, borrowed iterators, resource-derived handles, and
internal-storage views cannot escape their proven lifetime. The compiler tracks
these facts without public lifetime parameters. Explicit `Pin<T>` is not needed
for ordinary interop: validated native borrowing pins automatically for the
borrow's extent.

### Unsafe enables a closed set of operations

Pointer creation/dereference/arithmetic/casts, unsafe native calls, unchecked
native construction, untagged native-union access, weak atomic ordering, and
manual implementation of internal safety contracts require `unsafe`. Unsafe
functions remain auditable because dangerous expressions still appear in an
explicit unsafe block. It never disables types, scope, mutability, permissions,
or OS checks.

### Unsafe mutation is transactional where truthful

An ordinary unsafe block journals writes to Zirk-managed state and validated
native ranges. Controlled errors, exceptions, traps, and cancellation before
commit close newly acquired resources and roll those writes back. Implementors
may use first-write journals, copy-on-write, range snapshots, escape analysis,
static commits, and merged nested journals.

Irreversible operations require `commit {}`. Entering a commit region publishes
pending reversible writes and acknowledges that external I/O, unknown FFI,
volatile/device memory, manual release, or concurrently observable effects
cannot be rolled back. Raw pointer writes without proven provenance and extent
also require this boundary. Unsafe transactions cannot suspend, spawn work, or
expose tentative state to another execution context.

### Tasks are structured and logically typed

`task` creates a child of the current scope and returns `Task<T>`; `await`
returns exactly `T`. Scope exit cannot abandon children. An unhandled exception
cancels siblings, awaits cleanup, and propagates with secondary failures
suppressed. A `Result.Error` remains an ordinary completed value. Long-lived
services require an application root supervisor rather than general detach.

### Aggregation keeps failure policy explicit

`Task.all` requires success and cancels remaining work after the first unhandled
exception. `Task.first` returns the first completion and cancels the rest.
`Task.settled` never performs sibling failure cancellation and returns ordered
`TaskSettlement<T>` values (`Fulfilled`, `Rejected`, or `Cancelled`). This gives
the Promise-all-settled use case without confusing `Result.Error` with task
rejection.

### Selection waits fairly without owning the losing operations

`select` waits for the first ready task, channel operation, timer, or
cancellation signal. It executes one branch, does not implicitly cancel losing
operations, supports a nonblocking `default`, and chooses fairly when several
branches are ready. Channel closure is a ready outcome rather than an infinite
wait.

### Transfer and sharing are compiler-derived

The compiler derives internal `Transfer` and `Share` properties. Values and
projections copy; strict immutable references may share; mutable references
transfer when exclusive or must use `clone()`; synchronization-aware references
may share. These concepts do not appear in ordinary function annotations and
cannot be asserted unsafely by normal user code.

### Parallelism and threads remain distinct from tasks

Tasks express concurrent logical work. `parallel` expresses finite CPU work and
preserves input order for map-like operations; unordered work is explicit.
Reductions require associative combiners and may regroup operations. `thread`
is scoped OS execution for native affinity or blocking isolation. Legacy
blocking work uses a separate `task.blocking` pool.

### Safe code prevents data races, not all nondeterminism

Concurrent mutable access requires ownership transfer, channels, strict
immutability, structured locks, or supported atomics. Ordinary mutexes cannot
cross `await`. Weak atomic orderings require unsafe; sequential consistency is
the default. Completion order remains nondeterministic unless the program
coordinates it explicitly.

## Risks / Trade-offs

- **Transactional unsafe blocks add runtime cost** → Log only first writes,
  remove journals through escape/failure analysis, and snapshot bounded ranges.
- **Rollback can be misunderstood as recovery from corruption** → Document
  the exact controlled failures and require `commit` for unprovable effects.
- **Implicit transfer analysis can surprise users** → Emit diagnostics that
  identify the crossing boundary and suggest strict sharing, synchronization,
  or `clone()`.
- **Cancellation cleanup can delay scope exit** → Keep cancellation
  cooperative, surface stuck-task diagnostics, and discourage long shields.
- **Fair selection can have scheduler cost** → Specify absence of permanent
  starvation rather than a particular queue algorithm.
- **Parallel floating reductions can vary by grouping** → Document the rule
  and provide an explicit deterministic reduction variant.
- **Native pinning can impede memory compaction** → Bound automatic pinning
  to validated borrow scopes and diagnose escaping native views.

## Migration Plan

1. Establish the two new normative capability specs and syntax/type deltas.
2. Add canonical semantic chapters for memory/unsafe and concurrency.
3. Align language, runtime, stdlib, compiler, roadmap, handbook, reference, and
   agent context; remove or qualify earlier contradictory wording.
4. Add complete valid/invalid examples and navigation links.
5. Validate OpenSpec artifacts, links, Markdown fences, formatting, and local
   repository checks before commit.

## Open Questions

No semantic questions remain for this change. Concrete collector, scheduler,
journal representation, fairness algorithm, and platform-native mechanisms are
implementation decisions constrained by these observable requirements.
