## Why

The remaining Phase 5 APIs cannot be safely delivered as isolated runtime features: their cancellation, ownership, GC, and resource-boundary contracts are not yet complete. This change closes the phase only after the compiler can prove the advertised guarantees at every boundary and the runtime can uphold them under cancellation, collection, shutdown, and bounded-resource pressure.

## What Changes

- Define deterministic outcome and failure semantics for `Concurrent.of`, `Concurrent.each`, timeouts, and cancellation; require homogeneous result types unless a representable typed sum is introduced, and bound active work for collection combinators.
- Separate short-lived app-background work from supervised services; define ownership, startup, signal handling, ordered shutdown, error delivery, drain deadlines, and forced-exit behavior.
- Implement `Thread.run` on the bounded managed worker pool with cooperative join, typed callback/error status, stable caller roots, collector participation, and fail-closed target selection; keep unproven blocking FFI calls rejected.
- Make synchronization waits cooperative and cancellation-safe; specify fairness, reentrancy, poisoning, unwind, timeout, and resource-release behavior for each primitive.
- Replace the current `Concurrent.protect` design with synchronous, non-suspending cleanup protection. Cancellation remains terminal; the checker does not prove cleanup termination or a time bound, so cleanup code must terminate for cancellation to be delivered. No unresolvable-wait detector is assumed.
- Establish sound effect summaries for suspension and complete, non-forgeable `Transfer` / `Share` analysis at all concurrent boundaries before exposing APIs that rely on those guarantees.
- Define atomic supported types, widths, target fallbacks, memory-order validation, pointer lifetime/GC rules, and `compare_exchange` success/failure ordering constraints.
- Preserve existing structured-concurrency and phase-planning scenarios while updating their behavior for the new APIs. Assign the new decision record the next available ADR number (ADR-023).
- Deliver Phase 5 status only after compiler, runtime, GC, target, docs, and companion-site acceptance evidence is complete.

## Capabilities

### New Capabilities

- `concurrency-combinators`: bounded concurrency, typed outcomes, deterministic failure/cancellation behavior, and result ordering.
- `background-and-services`: explicit app-background and root-supervisor ownership and shutdown contracts.
- `raw-threads`: bounded native-thread execution integrated with cooperative scheduling and GC.
- `synchronization`: cooperative synchronization, scoped cancellation protection, and portable atomic semantics.

### Modified Capabilities

- `zirk-structured-concurrency`: refine supervision, thread, synchronization, and data-race contracts while retaining existing scenarios.
- `zirk-type-system`: define representable combinator result types, suspension effects, and sound boundary ownership checks.
- `zirk-ir-lowering` and `zirk-native-codegen`: add only the operations justified by the accepted semantics.
- `async-runtime-core`: specify bounded scheduling, background shutdown, thread lifecycle, and cooperative synchronization.
- `zirk-memory-safety`: explicitly reject pointer atomics until a rooting/lifetime protocol exists, define the GC-managed inline scalar payload, and specify unsafe memory-order obligations.
- `zirk-feature-phasing`: preserve prior delivery/defer/removal scenarios and mark Phase 5 complete only after every gate passes.

## Impact

This is a compiler, runtime, GC, type-system, and public-semantics change affecting `crates/zirk-sema`, `crates/zirk-ir`, `crates/zirk-codegen-llvm`, `crates/zirk-runtime`, fixtures, tests, handbook, status and roadmap documents. The implementation includes a cooperative executor, a CPU-parallel pool, and a bounded managed worker pool for native `Thread.run` callbacks. Managed callbacks register with the collector and retain caller roots until join; blocking FFI remains gated unless its effect and collector transition are proven. Existing cancellation cleanup deliberately makes blocking operations return when cancellation is pending, so protected cleanup must remain bounded and non-suspending.

The companion `../zirk-lang-site` is impacted by public documentation and implementation-status changes. Update it only after reviewed compiler-repository evidence exists, pin the exact compiler revision, review its status catalog, and record the explicit audit date and both repository revisions.
