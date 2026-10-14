## ADDED Requirements

### Requirement: Synchronization and combinator types are known

The checker SHALL recognize `Atomic<T>`, `Mutex<T>`, `RwLock<T>`, `Semaphore`,
`Barrier`, `Once<T>`, `Outcome<T>`, and the `Concurrent.of` pipeline as known
types. `Concurrent.of` SHALL take 2 to 8 homogeneous thunks `(): T` and return a pipeline; heterogeneous result types SHALL be rejected unless an explicit representable sum type is used, and `Void`/`Never` results SHALL be rejected because they cannot be carried as `Outcome<T>` payloads. `.first()` SHALL be typed `T`; `.settled()` SHALL be typed `List<Outcome<T>>`; `.within(d)` SHALL require `d: Duration` and return the
pipeline. `Concurrent.each(coll, fn)` SHALL be `List<R>` and
`Concurrent.each_settled(coll, fn)` SHALL be `List<Outcome<R>>`. `Thread.run(fn):
T` for `fn: (): T`. `Concurrent.detach(fn)` SHALL require `fn: (): Void` and be
typed `Void`. Collection combinators SHALL accept only `Array<T>` or `List<T>`
through an `inmut::strict` binding whose complete element graph satisfies
`Share`; a local-name transfer SHALL NOT suffice while aliases may remain.
Their optional integer limit SHALL be in 1..=256 (default 32), the collection
length SHALL be at most 256, and callback results SHALL reject `Void` and
`Never` before lowering.

#### Scenario: Concurrent.of first result type
- **WHEN** `Concurrent.of((): Value => a(), (): Value => b()).first()` is checked
- **THEN** the result has type `Value`

#### Scenario: settled result type
- **WHEN** `Concurrent.of(...).settled()` is checked over `Value` members
- **THEN** the result has type `List<Outcome<Value>>`

### Requirement: Mutex guard typing

`mutex.with(closure)` SHALL type `closure` as `(view: T): R` where the view is a
writable reference that SHALL NOT be assignable to a binding that outlives the
closure and SHALL NOT be captured by a nested `spawn` or returned. A safe point
inside the closure SHALL be a compile-time error.

The implementation SHALL represent `view` with a compiler-internal,
scope-indexed borrowed type rather than the ordinary by-value type `T`. The
borrowed type SHALL be rejected from assignments, returns, escaping captures,
pointer conversions, and calls whose effect or ownership summary is incomplete.
The checker SHALL derive the callback's suspension effect transitively before
the API is enabled. Until this scoped-view model exists in both sema and IR,
`Mutex<T>` SHALL remain behind the Phase 5 type gate. Once the scoped type
model exists but executable payload projection, runtime ABI, or all-exit unlock
lowering is incomplete, type checking SHALL still enforce the rules above while
construction and `mutex.with` report `NOT_LOWERED`; code generation SHALL NOT
emit an executable no-op lock or treat the handle as its payload.

#### Scenario: guard cannot escape
- **WHEN** a `mutex.with` closure assigns its view to an outer `mut` binding
- **THEN** compilation fails

#### Scenario: safe point under a guard
- **WHEN** a `mutex.with` closure calls a channel receive
- **THEN** compilation fails naming `RwLock<T>` or a redesign

### Requirement: Weak atomic ordering is unsafe-only

`AtomicOrder.relaxed`, `.acquire`, `.release`, and `.acq_rel` SHALL be accepted
only inside an `unsafe` context. Outside `unsafe`, an atomic operation SHALL use
`AtomicOrder.seq_cst`, which SHALL be the default.

#### Scenario: relaxed outside unsafe
- **WHEN** `counter.load(order: AtomicOrder.relaxed)` is checked outside `unsafe`
- **THEN** compilation fails
