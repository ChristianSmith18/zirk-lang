# ADR-023: Safe completion contracts for Phase 5 concurrency

## Status

Accepted

## Context

Phase 5 already has structured cooperative tasks, timers, bounded channels, a separate CPU worker pool, and a stop-the-world collector for registered participants. It does not yet have general `Transfer` / `Share` proofs or transitive suspension effects. The collector does not automatically coordinate arbitrary native threads. Cancellation-aware cleanup deliberately avoids waiting at blocking safe points after cancellation. The remaining concurrency APIs therefore depend on contracts that must agree across semantic analysis, IR, runtime, GC, and supported targets.

## Decision

### API exposure follows proof gates

No concurrency surface is marked implemented until its boundary checks, runtime behavior, target support, and regression evidence are complete. Unknown and unaudited external/native calls are conservative. Unsupported target capabilities produce explicit diagnostics.

### Combinators have bounded admission and representable results

`Concurrent.of` initially accepts homogeneous `(): T` functions. `first` returns the first successful result; failures before success are retained and sibling work is cancelled and joined. The earliest rejected completion sequence is primary; input order breaks a sequence tie or missing-sequence fallback. If a successful winner's losing branch fails during cancellation cleanup, the earliest cleanup failure becomes primary and later cleanup failures are suppressed in completion order. `settled` reports each outcome in input order. Timeout is distinct from caller cancellation. `each` and `each_settled` accept at most 256 collection elements, use a default concurrency of 32 or an explicit limit from 1 through 256, preserve input order, and do not eagerly start an unbounded collection. Invalid sizes or limits fail with `InvalidArgumentError` before callback admission. Their input collection must be held through an `inmut::strict` binding and its complete element graph must derive `Share`; transferring one local collection name is insufficient because aliases may remain in bindings, captures, fields, or views. Heterogeneous results require an explicit representable sum type.

Non-void language task results cross the scheduler ABI as pointers to
compiler-generated, descriptor-backed GC boxes. The box carries the exact
target LLVM layout of `T`; its descriptor traces every recursively nested
managed reference. Generated branch code allocates and roots the box before
calling user code, fills the payload without an intervening safepoint, and
retains it through the task control block until exactly-once consumption.
`JobWait` reloads the statically known `T` from the box. This avoids truncating
128-bit scalars or treating inline records/enums as pointers while retaining a
one-word scheduler result. Void-like results use zero. Managed outcome and
combinator APIs remain gated until cohort and failure semantics are complete.

Each executor admits at most 4,096 nonterminal tasks and 4,096 armed timers.
Saturation is surfaced as a typed `TaskCapacityError` or `TimerCapacityError`
through ordinary language exception propagation; the ABI never encodes a
failure as a task ID sentinel. Task capacity is released after terminal
completion (including completed cancellation cleanup), even while the joinable
outcome remains retained. Timer capacity is released exactly once on expiry or
disarm. Timer callbacks consume both task and timer capacity while active.

### Background work has explicit owners

`Concurrent.detach` is short-lived work owned by `App.background`; `application.spawn_service` is long-lived work owned by the root supervisor. Both have bounded admission, explicit error ownership, cancellation, ordered shutdown, and a deadline. Deadline expiry reports work that did not finish; it does not claim native threads can be forcibly terminated. Generated CLI programs exit with status 124. Embedded hosts choose `zirk_rt_run_main_with_deadline_policy` to wait safely or use the same exit status. Error-handler failure is captured and reported without recursively aborting shutdown.

The public service contract is `application.spawn_service(fn(ServiceContext): Void)`. Every source service is required and admission is capped at 64 services per application. Registration is accepted from managed application tasks until draining begins and immediately schedules the callback when all earlier required services are ready; each service depends on all required services registered before it. The service calls `ServiceContext.ready()` only after its required startup succeeds. Returning or throwing before that call is startup failure; returning normally or throwing after readiness is an unexpected runtime failure and starts application drain. A request handler may return while the application remains active; completion of `main` or host shutdown ends the application lifecycle. Application readiness is published only after every required service is ready. Shutdown cancels and drains services in reverse actual startup order; service captures and task roots remain owned by the supervisor through cleanup. Services and detached background jobs share one grace period and the same host expiry policy. Source errors distinguish `ServiceCapacityError`, `TaskCapacityError`, and `ServiceLifecycleError`.

The app supervisor follows this lifecycle:

The independent background quota is 256 active jobs and rejects saturation
with `BackgroundCapacityError`. If the executor's separate 4,096-task quota is
reached first, admission reports `TaskCapacityError`; the source runtime ABI
keeps these causes distinct. A terminal background job releases its slot.

| Current state | Event | Next state | Required behavior |
|---|---|---|---|
| `Created` | first service is registered | `Starting` | initialize the supervisor and immediately schedule eligible services |
| `Starting` or `Ready` | another service is registered | unchanged or `Starting` | depend on all earlier required services and schedule when eligible |
| `Starting` | all required services report ready | `Ready` | publish readiness only after every required service is ready |
| `Starting` | service startup fails or startup is cancelled | `Draining` | stop already-started services in reverse startup order and report the startup failure |
| `Ready` | host shutdown signal or `main` completion | `Draining` | stop accepting new work, cancel `App.background`, and stop services in reverse startup order |
| `Draining` | owned work completes before the grace deadline | `Stopped` | release runtime roots and emit final diagnostics |
| `Draining` | grace deadline expires | `Stopped` | report each unfinished job/service; retain roots until native participants actually exit and apply the host exit policy |

Readiness is a one-way transition: a required service failure before `Ready`
fails startup. After `Ready`, service failure is reported to the supervisor and
initiates application shutdown unless an explicit restart policy is configured.
The host supplies shutdown signals and selects the final process-exit policy;
Zirk can cooperatively cancel work but cannot forcibly terminate a native
thread. Generated CLI programs use exit status 124 at grace expiry; embedded
hosts can select safe wait or that same process-exit behavior.

### Native work is managed and GC-coordinated

`Thread.run` uses bounded managed OS-thread admission, not an unbounded thread-per-call policy. Threads that may hold Zirk values publish stable roots and participate in collector coordination. The collector's participant registry is protected by the same admission gate as the stop-the-world request: registration cannot enter the published heap after a collection snapshots participants, and unregistration cannot release a root chain until that collection finishes. Each participant is `Running`, `Parked`, or `InNative`; collection waits until every participant is quiescent, scans every registered shadow-frame chain, then clears the request and reopens admission.

A thread entering a blocking native call must first publish its roots and transition to `InNative` under that gate. While in that state it cannot access managed objects or call back into Zirk/runtime code. Returning from native code re-enters through the gate and waits for any active collection before touching the heap. This contract does not make arbitrary callbacks or retained raw pointers safe; they require pinned handles or a separate audited ownership protocol. Mutator registration is RAII-managed, and unwind cleanup must remove dead shadow frames before the next safepoint. Caller cancellation retains thread-owned roots and runtime state until join or the documented shutdown deadline. Targets without the full protocol reject the feature.

### Protected cleanup is non-suspending but not time-bounded

`Concurrent.protect` runs synchronous cleanup that does not suspend. Cancellation remains pending and is delivered after cleanup. If cleanup throws while cancellation is pending, cancellation remains primary and the secondary pending throwable is consumed; absent cancellation, the cleanup throwable propagates normally. Channel/timer/task/thread waits are not made immune to cancellation; transitive suspension is rejected. No unresolvable-wait detector is assumed. The effect checker rejects incomplete effects and known suspension, blocking, or spawning, but does not prove termination or a time bound; a non-suspending loop can delay cancellation indefinitely. Lowering brackets the cleanup callback with nesting-safe shield-depth changes and checks cancellation before exception dispatch after restoration. Native cancellation execution coverage remains required.

### Synchronization is scheduler-integrated

Mutexes and other waits park tasks through scheduler wait queues and never block the cooperative executor. Mutex is FIFO and non-reentrant. Cancellation removes waiters safely; unlock/release occurs on all exit paths. RwLock is non-reentrant and writer-aware: waiters remain FIFO, later readers cannot bypass a queued writer, and contiguous readers at the queue head are admitted as a cohort. It has no poisoning, upgrade, or downgrade API; changing from read to write access requires a fresh request after releasing the read view. Semaphore permits are bounded, FIFO, and RAII-released; close is idempotent and wakes every waiter with a closed result. A fixed-participant Barrier permanently breaks and wakes its generation if any waiter is cancelled. Once publishes only a fully initialized value; an initializer panic publishes nothing, propagates to its caller, and allows one waiter to retry. No poisoning is exposed unless recovery semantics are specified.

#### Mutex source contract and implementation

`Mutex<T>.with(callback)` cannot be represented as an ordinary callback
parameter of type `T`: that would allow the protected view to be copied,
returned, stored, or captured after unlock. The compiler carries an internal
scope-indexed borrowed-view type through sema and IR, rejects such escapes,
and proves direct and transitive callback effects are non-suspending,
non-blocking, and non-spawning. Lowering retains GC roots for both the mutex
and callback while acquisition is queued; the runtime's RAII guard unlocks
before propagating callback return, a Zirk Throwable, cancellation while
waiting, or a native unwind.

The minimum compiler representation is an interned `Borrowed { scope, referent }`
entry in sema; a borrow must not be represented by reusing `referent` or by
lowering to an ordinary pointer. `scope` identifies one lexical callback
invocation (not merely a function signature), so sibling and nested `with`
calls cannot accidentally unify. The callback parameter receives this type,
and all type-acceptance, inference, union, generic substitution, capture, and
diagnostic paths must preserve it. Sema rejects assignment to an outer binding,
return, storage in fields/collections, pointer or native-slice conversion,
capture by a closure that can outlive the callback, and passage to a call whose
no-escape and non-suspending summaries are not complete. Reads and writes are
only through operations scoped to this borrow.

IR must preserve the same identity in a dedicated borrowed-view type and a
borrowed-view value produced by the mutex callback entry. The IR verifier, not
only sema, rejects storing that value into a heap/object/aggregate, returning
it, capturing it in an escaping callable, passing it to an unverified call, or
using it outside its callback region. The IR schema/portable package version
must be updated if adding the type changes serialized IR. Lowering must keep the
mutex and callback roots live across queued acquisition and place unlock on
every normal, return, exceptional, and cancellation edge. This representation
is a capability to access the payload, not an address that general pointer
operations can consume.

The managed mutex state and protected payload have a per-object lifetime
tied to the GC allocation and a descriptor-aware tracing path for references
nested in `T`; a process-global native-handle registry is not an acceptable
substitute. Waiter queue state must remain reachable while the object is live,
and the object destructor must release any Rust-owned queue storage when the
GC sweeps it. The source implementation uses this managed allocation and the
cooperative FIFO mutex runtime. Borrowed-view verification permits only a
narrow, audited set of non-allocating metadata getters while the view is live;
unknown runtime calls remain rejected. The mutex is non-reentrant and does not
expose poisoning, upgrade, or downgrade behavior.

### Effects and ownership are checked transitively

The checker derives `may_suspend` / `may_block` / `may_spawn` through direct, closure, generic, virtual/contract, and external calls. These effects propagate transitively to protect blocking CPU-only regions such as `parallel`. Unknown/external calls are conservative unless a validated contract exists. `Transfer` / `Share` derives from the reachable type graph, aliases, mutability, resources, and synchronization contracts, tracks moves/use-after-transfer through control flow, and is enforced at every concurrent boundary. These are prerequisites to enabling dependent APIs.

### Atomics have portable, explicit contracts

`Atomic<T>` stores its supported scalar payload inline in its GC-managed object allocation, with target-correct alignment and a descriptor that treats the payload as non-reference data. Collection of the Atomic object ends the payload lifetime; no separately allocated native handle or explicit free is exposed to Zirk code. The source API is `Atomic(initial)`, `load`, `store`, `exchange`, `compare_exchange`, and integer-only `fetch_add` / `fetch_sub`. Compare-exchange returns `Result<T,T>` containing the observed value in either variant and validates success/failure ordering pairs. All operations default to sequential consistency; weaker orderings require `unsafe`. Loads accept only relaxed/acquire/SeqCst, stores relaxed/release/SeqCst, and read-modify-write operations accept all orderings. Statically known invalid combinations are compilation errors; dynamic invalid combinations raise `InvalidArgumentError` before access. Pointer atomics, including managed pointers, remain unsupported. Supported scalar widths are enumerated per target; lock-free behavior is claimed only with target evidence. Alignment and wrapping arithmetic behavior are documented. The existing Rust/C `AtomicCell` ABI remains a runtime-only reference implementation and does not define source-object ownership or lifetime.

## Boundary matrix

| Boundary/API | Effect rule | Transfer/Share | Cancellation and ownership | GC/resource rule |
|---|---|---|---|---|
| `Concurrent.of` / `each*` | callbacks may suspend only through structured executor | `each*` collection requires `inmut::strict` plus deep `Share`; callback captures/results checked | siblings cancelled and joined; caller cancellation remains cancellation | finite active-work limit; ordered outcomes |
| `spawn` / `concurrent` | inferred task effects | captures and returned values checked | parent scope owns children and cleanup | task/timer quotas; suspended roots retained |
| `Timer` | sleep and waits are suspension effects | callback captures/results checked | cancellation wakes and removes timer waiter | bounded timer registrations; no leaked timer after timeout |
| `Channel<T>` | send/receive are safe points | channel values/endpoints checked | cancelled waiters removed; close/drain rules preserved | bounded queues and traced queued values |
| `parallel` | no suspension or blocking I/O | current parallel boundary plus complete ownership proof | structured join | registered pool workers only; collector safepoints |
| `Thread.run` | no nested suspend/task APIs; native blocking allowed by contract | entry and result boundary checked | managed worker survives caller cancellation through join/deadline | bounded pool, stable roots, collector participant protocol |
| `Mutex` / RwLock | no transitive suspension while guard held | synchronized sharing only under checked handle | FIFO wait; cancellation-safe dequeue; release on all exits | no OS blocking of executor |
| Semaphore / Barrier / Once | wait effects explicit | shared handle contract checked | cancellation/close/failure cannot strand peers | bounded permits and waiter queues |
| `Atomic<T>` | non-suspending | only supported scalar; pointer atomics unsupported | independent of task cancellation | target width/alignment/order validated; managed pointer excluded |
| `Concurrent.protect` | must not suspend or block | no new boundary | pending cancellation delivered after synchronous cleanup; no termination/time bound is proven | no wait detector; no unbounded shield |
| `App.background` | callback effect checked | values constrained to background lifetime | app owns cancel/drain and error handler | bounded admission and shutdown grace |
| `application.spawn_service` | service lifecycle contract | service ownership transfers to root | root supervisor owns readiness/failure/ordered stop | bounded service count and explicit deadline |

## Failure and threat matrix

| Condition | Required result | Test owner |
|---|---|---|
| Unbounded collection or task creation | reject or apply documented finite bound/backpressure | combinator/runtime |
| First-result simultaneous completion/failure | deterministic primary outcome; remaining failures suppressed | combinator/scheduler |
| Caller cancellation races timeout | caller cancellation stays cancellation; no timer leak | runtime |
| Cancellation while waiting for lock/channel/barrier | waiter removed or terminal cancellation; no lost wakeup | scheduler/synchronization |
| Panic during protected cleanup or mutex closure | restore shield/lock state on every unwind edge | IR/runtime |
| GC during running or blocked managed thread | roots remain valid; collector progresses | runtime/GC per target |
| Native call never returns | deadline reports unfinished work; runtime roots retained until host exit | thread/shutdown |
| External call has unknown effects | conservative reject where suspension/thread safety matters | sema |
| Pointer atomic references managed object | reject unless audited rooting/lifetime protocol exists | sema/runtime/GC |
| Background handler throws recursively | capture and report; shutdown state remains coherent | app supervisor |
| Target lacks atomic or GC thread support | explicit capability diagnostic | target/codegen |
| Shutdown grace expires | report unfinished jobs and follow documented host exit policy | app supervisor |

## Consequences

### Delivered runtime subset

The current parallel worker pool caps native workers at 64, including when
`ZIRK_PARALLEL_CORES` requests more, and bounds its pending-job queue to
`min(4 × configured_workers, 1,024)`. A full queue applies blocking
backpressure; executor-task submitters use timed waits that poll collector
safepoints. This bounds queued jobs, not eager chunk materialization or input
size. The background supervisor retains at most 1,024 recent diagnostics,
evicting the oldest entry when a new one is recorded.

If detached-background shutdown grace expires while a parallel region remains
active, the runtime reports unfinished jobs but continues safepoint service and
joins the outstanding region before unpublishing or freeing the shared heap.
The deadline is therefore diagnostic for this path, not a mechanism for
forcibly terminating workers. A non-returning worker can keep shutdown waiting
indefinitely. These guarantees apply to the delivered parallel pool only.

`Thread.run(fn)` is implemented on supported native 64-bit targets through the
bounded managed pool. It preserves caller roots until join, registers the
worker with the collector, and turns callback exceptions into caller-side
throws. The pool has a 64-worker process-wide ceiling and a 1,024-callback
queue; a full queue applies cooperative backpressure. Codegen rejects Wasm and
32-bit pointer ABIs. Blocking FFI remains unavailable until the generated
`InNative` transition is tied to a complete effect contract. The distinct
`Thread<T>` resource API and language synchronization primitives remain
unavailable.

- The effect and ownership analyses are foundational work and gate API rollout.
- `Thread.run` is rejected on targets without the managed-thread/collector ABI; blocking FFI remains rejected until its `InNative` transition is proven.
- Bounded admission may trade peak throughput for predictable memory and thread usage; limits are benchmarked only after correctness.
- Phase 5 completion is evidence-based and target-scoped.

## Related

- [ADR-017](./ADR-017-modelo-de-suspension.md) — cooperative executor.
- [ADR-019](./ADR-019-parallel-and-multithreaded-gc.md) — worker-pool collector coordination.
- [ADR-018](./ADR-018-concurrency-surface.md) — delivered structured-concurrency surface.
- `openspec/changes/concurrency-completion/design.md` — implementation sequence.
