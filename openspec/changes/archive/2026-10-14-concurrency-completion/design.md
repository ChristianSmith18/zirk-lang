## Context

See proposal.md for motivation. The delivered runtime has a cooperative single-thread executor for structured I/O, a distinct fixed OS worker pool for CPU `parallel`, and a stop-the-world collector that registers those known participants. Cancellation-aware cleanup treats pending cancellation as terminal: timer and channel waits do not become blocking merely because cleanup is running. The semantic checker currently has a parallel-capture check, not a complete `Transfer` / `Share` proof or general transitive suspension-effect analysis. Existing public specs contain scenarios that must remain intact when requirements are modified.

## Goals / Non-Goals

**Goals:** Establish implementable contracts before enabling APIs; ensure cancellation, GC, ownership, and resource bounds agree across sema, IR, runtime, and docs; split work into independent vertical slices with explicit gates.

**Non-Goals:** Replacing the cooperative executor with a general multi-thread scheduler; creating unbounded threads or tasks; allowing cancellation shields to suppress cancellation indefinitely; claiming safe-code data-race freedom without complete checker evidence; changing already delivered `concurrent`, `parallel`, or `Channel<T>` behavior except where a clarified shared contract requires it.

## Decisions

### D1: Contracts and gates precede implementation

The first implementation gate is strict OpenSpec validation plus the boundary and threat matrices in [ADR-023](../../../docs/decisions/ADR-023-concurrency-completion-contracts.md). Preserve every still-current scenario from the base specs. No API or Phase 5 status may be marked delivered until its relevant checker, runtime, GC, target, and stress evidence passes. ADR-023 is the next available decision record; ADR-020 through ADR-022 already exist.

### D2: Combinators use representable result types and bounded admission

`Concurrent.of` initially accepts homogeneous `(): T` thunks, so `first(): T` and `settled(): List<Outcome<T>>` are sound in a language without union types. A future heterogeneous form requires a declared enum/sum representation and explicit conversion rules; do not infer a fictitious common type. `first` means first successful completion; if a member fails before any success, retain the earliest failure by executor completion sequence as primary, cancel and join siblings, and attach later failures as suppressed. Input order breaks a sequence tie (or a missing-sequence fallback). If the winner succeeds but a losing branch fails during cancellation cleanup, reject with the earliest cleanup failure as primary and attach subsequent cleanup failures as suppressed in completion order. If all branches fail, the same completion-sequence rule selects the primary. For `.settled()`, a branch that terminates as cancelled or throws `CancelledError` produces `Outcome.Cancelled`; throwing `CancelledError` is the source-level per-branch cancellation signal. Its `finally` cleanup completes before the branch outcome is collected. If cleanup throws a different error, the branch instead produces `Outcome.Rejected` with that cleanup failure. Caller cancellation remains operation-level cancellation. Timeout is a distinct timeout failure. Test the precise selection and suppression order.

`Concurrent.each*` accepts only `Array<T>` and `List<T>` with at most 256 elements, checked before any callback starts. Its collection must be held through an `inmut::strict` binding and its complete element graph must satisfy `Share`; transferring one local name is insufficient while aliases may remain in bindings, captures, fields, or views. Its optional third positional argument sets concurrency to 1..=256; the default is 32 and the effective limit is capped by the collection length. Empty collections return empty lists. Both APIs preserve input order and enforce the existing `Transfer`/`Share` callback capture/result boundaries; callback results cannot be `Void` or `Never`. `each` fails fast, cancels and joins admitted siblings, then propagates the primary failure. `each_settled` records one `Outcome<R>` for every callback. Caller cancellation cancels and joins all admitted callbacks and propagates cancellation without a partial list. Invalid sizes and limits raise `InvalidArgumentError` before callback admission.

### D3: Background jobs and services have distinct owners

`Concurrent.detach` is short-lived, best-effort work owned by `App.background`; `application.spawn_service` is long-lived work owned by the application supervisor. Every source-level service is required and the app admits at most 64. `spawn_service` registers and schedules a service without waiting for it; each service automatically depends on all previously registered required services, so registration order is readiness dependency order. Registration remains available to managed application tasks until draining begins. A service receives `ServiceContext` and must call `ready()` after required startup work succeeds; returning or throwing before that is startup failure, and returning normally or throwing after readiness is an unexpected runtime failure. A handler can return while the application remains active; only completion of `main` or host shutdown ends the application lifecycle. The supervisor publishes application readiness after all required services report ready and drains started services in reverse startup order. Service captures and task roots remain supervisor-owned through cleanup. Services and `App.background` share one shutdown grace period; on expiry, report remaining jobs/services and preserve roots for live native participants. Generated CLI entrypoints then exit with status 124; embedded hosts retain control through `zirk_rt_run_main` and apply their own host policy. The runtime never frees memory while a live native callback may still reference it. Handler failures are captured and reported without recursively destabilizing shutdown.

The native executor exposes `App.set_shutdown_grace(Duration)` from the root program before drain begins. It updates the current executor instance; five seconds remains the default, zero is valid, and negative or post-drain updates raise `InvalidArgumentError`. The browser interpreter has no application executor and excludes this API with the managed-background capability gate.

### D4: Native-thread execution is bounded and collector-aware

Do not implement a fresh unconstrained OS thread per call. Use a bounded blocking executor or bounded admission to a managed native-thread pool, with explicit queue/backpressure and cancellation policy. Every worker that can hold Zirk references registers/unregisters roots and participates in stop-the-world coordination. A blocking FFI call that cannot poll must enter a runtime blocking/park protocol that safely publishes roots and marks the thread as blocked; collection must neither wait forever for its poll nor scan an unstable stack. Define ownership while the caller is cancelled, shutdown behavior, panic/error conversion, and runtime lifetime. If these guarantees cannot be established for a target, reject `Thread.run` there rather than silently weakening GC safety.

### D5: `Concurrent.protect` only supports non-suspending cleanup

Protection is a cleanup construct, not a way to defer cancellation across arbitrary waits. It runs synchronous, non-suspending cleanup while cancellation is pending; safe-point operations such as channel send/receive, timers, `spawn`, and `Thread.run` are rejected within the protected closure. The checker does not prove termination or a time bound, and the runtime does not preempt a non-suspending loop, so cleanup must terminate for cancellation to be delivered. Preserve the runtime invariant that cancellation is terminal and cleanup cannot park indefinitely. Remove the unresolvable-wait-detector claim unless a separately designed detector has a sound implementation and boundedness proof. Nested protection is allowed with depth-safe restoration on every return/exception edge; forceful cancellation does not bypass cleanup but a global deadline may terminate the process under the host shutdown policy.

When cleanup throws while cancellation is already pending, cancellation remains the terminal outcome and the secondary pending throwable is consumed before the task exits; when cancellation is absent, the cleanup throwable propagates normally.

### D6: Synchronization is cooperative at task boundaries

Locks and permits must suspend a task through scheduler wait queues, never block the sole cooperative executor. Define FIFO or explicit fairness, cancellation removal from wait queues, wakeup races, closure/unwind release, and behavior on panic. Mutex/RwLock are non-reentrant and expose no poisoning. RwLock uses FIFO writer-aware admission: later readers cannot bypass a queued writer, and contiguous readers at the queue head enter as a cohort. There is no upgrade/downgrade operation; a read-to-write transition releases and separately reacquires, allowing intervening updates. Semaphore permits are bounded, FIFO, RAII-released, and closeable; close wakes all waiters and stays terminal. Barrier has a fixed nonzero participant count and becomes permanently broken if a participant is cancelled mid-generation. Once publishes only fully initialized values; initializer panic propagates, publishes nothing, and permits retry by a waiter. APIs that cannot satisfy these contracts on a target are unavailable there.

### D7: Suspension effects are transitive and checker-enforced

Add function effect summaries for `may_suspend`, `may_block`, and `may_spawn`, plus any required `thread_safe` effect. Infer summaries through calls, closures, generics, and virtual/contract dispatch; unknown or external calls are conservative unless declared with a verified contract. Reject a potentially suspending call in `Mutex.with` and `Concurrent.protect`. If whole-program inference cannot establish safety, require explicit effect annotations and validate overrides and extern declarations. Do not rely on a lexical scan of direct calls.

### D8: Ownership analysis is a prerequisite, not a final checkbox

Derive `Transfer` and `Share` from the complete reachable type graph, mutability, aliases, resources, and synchronization contracts. Track move/use-after-transfer state and joins through control flow. Enforce separately at branch capture/result, channels, parallel captures/results, thread boundaries, and shared synchronization handles. Treat unknown/native/extern types as non-transferable and non-shareable absent an audited contract. Gate every concurrency API on the relevant boundary analysis; retain legacy single-thread behavior only where it does not falsely claim race freedom. Unsafe escape hatches remain explicit and documented.

### D11: Every concurrency capability has an explicit compiler gate

The checker must either route an API through an implemented contract or reject
it at a named Phase 5 boundary. Runtime-only foundations do not enable a source
API. Keep the following matrix aligned with checker behavior and CLI/sema
fixtures. Built-in namespace routing applies only when no value or
user-declared class shadows that name; static methods on a same-named user
class continue through ordinary member resolution.

| Source capability | Current checker behavior | Remaining enablement proof |
|---|---|---|
| `concurrent {}`, `spawn`, and `Job<T>` | Delivered through structured-scope lowering; spawn captures/results are ownership-checked, scope summaries record known suspension/spawn effects, and direct `Job.wait()`/`cancel()` calls enforce linear handle use. | Preserve the current runtime and cancellation/cleanup contracts. |
| `Timer.sleep`, `Timer.after`, and `Timer.every` | Delivered. All validate positional arity and `Duration`; `sleep` records suspension, while `after`/`every` record child creation and require a zero-argument lambda literal (`every` returns `Void`). Timer callbacks check capture and result ownership before producing `Job<T>`. Their callbacks are cooperative branches and are not restricted to non-suspending effects. `after`/`every` branches are ambient children cancelled with their enclosing scope. | Preserve timer cancellation, ambient-scope cleanup, and result-root contracts. |
| `Channel<T>` construction and methods | Delivered: capacity constructor, `Channel.unbounded(limit: ...)`, `send`, `receive`, `try_send`, `try_receive`, `close`, and `drain`. Send ownership and receive-result boundaries are checked; operations return typed values/outcomes. | Preserve channel cancellation, close, and value-root contracts. |
| `parallel`, `.parallel`, and `Parallel.each` | Delivered as CPU-only work; parallel capture/result constraints are checked. | Preserve pool admission, collector participation, and backpressure contracts. |
| `Concurrent.of(...)` | Validates arity, positional closure literals, zero-argument signatures, homogeneous results, captures, and result ownership; then reports `NOT_LOWERED` (`E0423`). | Managed outcome/result/error rooting, pipeline IR, terminal lowering, cleanup, and ABI. |
| `Concurrent.of(...).first/settled/within` | Direct homogeneous thunk pipelines and both terminals are enabled through sema, IR, LLVM, and runtime. `.first`/`.settled` validate zero arguments and `.within` validates one `Duration`; thrown `CancelledError` becomes a per-branch `Outcome.Cancelled` in `.settled()` after cleanup. | Preserve rooted managed outcomes, exhaustive status handling, deterministic selection, cancellation, and timeout behavior. |
| `Concurrent.each` and `Concurrent.each_settled` | Enabled through sema, bounded dynamic IR lowering, LLVM, and the native scheduler. Validate Array/List, maximum size 256, `inmut::strict` deep-Share ownership, callback/result/limit shape, default concurrency 32, and explicit limit 1..=256 before admission; preserve input order and use rooted result staging. | Preserve bounded admission, validation-before-start, GC roots, fail-fast cancellation/join, ordered settled outcomes, and no partial list on caller cancellation. Browser execution is explicitly excluded. |
| `Concurrent.protect` | Enabled: requires a zero-argument `Void` lambda and complete transitive non-suspending/non-blocking/non-spawning effects; cancellation remains terminal and is checked after cleanup restores the shield. The checker does not prove termination or a time bound. | Preserve cleanup-edge ordering, cancellation priority, and nesting safety; add native execution/cancellation assertions. |
| `Concurrent.detach`, `App.set_shutdown_grace`, and `application.spawn_service` | `Concurrent.detach` is enabled for bounded application work; native `App.set_shutdown_grace(Duration)` configures one executor grace for background jobs and services (default five seconds). Service callbacks are required, start on registration, and wait on earlier required services' readiness. Browser execution excludes app-level background work and services. | Preserve lifecycle/effect/ownership gates, deadline reporting and participant lifetime; finish the source-to-runtime service path. |
| `Thread<T>` and `Atomic<T>`, `Mutex<T>`, `RwLock<T>`, `Semaphore`, `Barrier`, `Once<T>` | `Thread.run(fn)` is enabled on supported native 64-bit targets through sema, IR, LLVM, and the managed runtime; `Thread<T>` construction and language synchronization primitives retain Phase 5 `E0402` gates. `Thread.run` enforces complete effect summaries, rejects suspension/spawn and unsafe capture/result boundaries, and target codegen fails closed on Wasm and 32-bit pointer ABIs. | Preserve managed callback roots/join, target gating, and typed failures; finish blocked-FFI transitions and the remaining synchronizer/guard language integration. |
| `Outcome<T>` | Known generic enum with constructible variants; no completion API returns it yet. | Managed completion pipeline and `Result.Error` versus thrown-failure conversion. |
| Internal scalar `AtomicCell` ABI | Available as an IR/runtime C ABI foundation for bool and supported integer widths; not a source-language `Atomic<T>` capability and carries no pointer or lock-free guarantee. | Source type/lowering, unsafe-only weaker ordering, target operation proofs, and GC-safe pointer policy. |

Tests must assert each unavailable public entry point produces its named phase
gate rather than silently reaching an unknown member, generic call, or backend
failure. A gate is removed only with the corresponding row's enablement proof.

### D9: Atomic behavior is portable and pointer-safe

Enumerate exact supported scalar widths per target and define a portable fallback or a compile-time unsupported diagnostic. Validate operation-specific ordering legality (`load`: relaxed/acquire/seq_cst; `store`: relaxed/release/seq_cst; read-modify-write: all orderings) and `compare_exchange` success/failure pairs before memory access. Statically known invalid orders are compile errors; dynamic invalid orders raise `InvalidArgumentError` before access. Pointer atomics require explicit unsafe access and lifetime rules, cannot silently root/unroot GC objects, and cannot create a safe dangling reference; initially exclude managed GC pointers unless a handle/root protocol is designed. Define alignment, lock-free guarantees (do not promise lock freedom unless verified), and numeric overflow behavior.

### D10: Parallel work packages share one contract owner

Partition implementation by independent vertical slices: (A) effects + Transfer/Share; (B) scheduler/combinators; (C) app supervisor; (D) GC/thread integration; (E) sync/atomics; (F) IR/codegen integration; (G) docs/site and cross-cutting validation. A contract owner lands specs/design first; agents work in isolated files/worktrees and integrate through agreed interfaces. Do not parallelize edits to shared checker/runtime interfaces without a reviewed interface task. Merge in dependency order and rerun integrated evidence after every interface change.

## Risks / Trade-offs

- **Conservative effects reject some safe programs** → begin sound and conservative; improve precision only with tests proving override, external, and generic-call behavior.
- **Bounded thread/task admission reduces peak throughput** → expose explicit limits and backpressure; benchmark after correctness and report throughput/latency/resource ceilings.
- **GC registration and blocking FFI are target-sensitive** → capability-gate targets and test collection while threads are running, blocked, cancelled, and shutting down.
- **Full ownership analysis is compiler-wide** → implement it as a prerequisite work package; do not fake completion by extending only the existing parallel check.
- **Shutdown deadlines can abandon cleanup** → report incomplete work, define process behavior, and never imply forcible thread cancellation.
- **Website status can overstate delivery** → sync only from reviewed, committed source evidence and an explicit audit date.

## Migration Plan

1. Restore omitted base-spec scenarios, assign ADR-023, and validate all deltas strictly.
2. Finalize result algebra, bounds, effects, ownership, thread/GC protocol, sync semantics, target policy, and shutdown contract; produce a threat/failure matrix and API decision record.
3. Implement and test effect summaries and `Transfer` / `Share` independently; keep dependent APIs phase-gated until proven.
4. Implement bounded combinators and app supervision on the current cooperative executor.
5. Implement managed thread integration only after collector participation, FFI blocking protocol, cancellation, and shutdown tests pass on each supported triple.
6. Implement cooperative synchronization and portable atomics after ownership/effects gates.
7. Integrate IR/backend, fixtures, differential tests, stress tests, docs, and status evidence.
8. Run focused and workspace tests, target matrix, sanitizers/Miri where applicable, strict Clippy/format, strict OpenSpec, and resource/cancellation/GC stress. Update the companion website from the committed source revision, review status copy, record date and revisions. Only then mark Phase 5 complete.

## Open Questions

None that change the contract are deferred. Host-specific forced-exit behavior and the exact supported atomic widths must be settled in the ADR/API matrix before the corresponding implementation tasks begin.

### D12: Managed task results travel in descriptor-backed result boxes

The task scheduler ABI continues to exchange one machine word, but that word
is always an opaque managed result-box pointer for non-`Void`/non-`Never`
language results. The compiler emits a box layout containing the ordinary
three-word GC header followed by the exact LLVM representation of `T`; its
descriptor contains byte offsets for every managed reference reachable
recursively through `T` (including nested records, enums, closures, and
nullable values). The branch thunk allocates and roots a zeroed box before
calling user code, then stores the returned value into its payload without an
intervening allocation or safepoint. The task control block retains the box
pointer until its result is consumed. `JobWait` consumes that pointer and
loads the payload as the statically verified `T`; it must not reinterpret an
aggregate as a pointer or reconstruct it from a truncated word. Void-like
results retain the zero-word representation.

This keeps the scheduler ABI bounded while making the payload's GC map
compiler-generated and identical to ordinary object field tracing. The current
LLVM result-box emitter is supported only on 64-bit pointer targets; it fails
closed when a task-result descriptor is emitted for a target whose pointer
width differs from the collector's fixed-width metadata ABI. Do not infer 32-bit
support from object emission alone. Descriptors are trusted compiler-generated
static metadata, not a validated format for untrusted native producers. A
result box is not returned to user code and has no language identity. The
existing `Job<T>` result path is enabled; new `Outcome<T>` and combinator APIs
must use the same typed payload plus recursive root map, and remain gated until
allocation/rooting, exceptional completion, exactly-once consumption, and
cleanup are covered end to end. Rust-only combinators remain restricted to
unmanaged `Send` values.
