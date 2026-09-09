## ADDED Requirements

### Requirement: Synchronization and combinator types are known

The checker SHALL recognize `Atomic<T>`, `Mutex<T>`, `RwLock<T>`, `Semaphore`,
`Barrier`, `Once<T>`, `Outcome<T>`, and the `Concurrent.of` pipeline as known
types. `Concurrent.of` SHALL take 2 to 8 thunks `(): Ti` and return a pipeline;
`.first()` SHALL be typed as the common type of the members; `.settled()` SHALL be
typed `List<Outcome<T>>`; `.within(d)` SHALL require `d: Duration` and return the
pipeline. `Concurrent.each(coll, fn)` SHALL be `List<R>` and
`Concurrent.each_settled(coll, fn)` SHALL be `List<Outcome<R>>`. `Thread.run(fn):
T` for `fn: (): T`. `Concurrent.detach(fn)` SHALL require `fn: (): Void` and be
typed `Void`.

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

#### Scenario: guard cannot escape
- **WHEN** a `mutex.with` closure assigns its view to an outer `mut` binding
- **THEN** compilation fails

#### Scenario: safe point under a guard
- **WHEN** a `mutex.with` closure calls a channel receive
- **THEN** compilation fails naming `RwLock<T>` or a redesign

### Requirement: Weak atomic ordering is unsafe-only

`AtomicOrder.relaxed`, `.acquire`, and `.release` SHALL be accepted only inside an
`unsafe` context. Outside `unsafe`, an atomic operation SHALL use
`AtomicOrder.seq_cst`, which SHALL be the default.

#### Scenario: relaxed outside unsafe
- **WHEN** `counter.load(order: AtomicOrder.relaxed)` is checked outside `unsafe`
- **THEN** compilation fails
