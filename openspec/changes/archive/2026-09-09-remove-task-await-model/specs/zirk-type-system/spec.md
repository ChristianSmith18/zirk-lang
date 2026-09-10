## REMOVED Requirements

### Requirement: Async core types are known
**Reason**: `Task<T>`, `Channel<T>` as an async handle, and `TaskSettlement<T>` are removed from the async model.
**Migration**: `Job<T>` (from `spawn`) and `Timer` — `concurrent-blocks-and-timers`; `Channel<T>` — `typed-channels`; `Outcome<T>` — `concurrency-completion`.

### Requirement: `task` produces a typed handle and `await` produces the element type
**Reason**: `task` and `await` are removed.
**Migration**: `spawn expr : Job<T>`, `job.wait() : T` — `concurrent-blocks-and-timers`.

### Requirement: A task result is consumed exactly once
**Reason**: no `Task<T>` handle.
**Migration**: `Job<T>` is single-consume: a second `job.wait()` is a compile error — `concurrent-blocks-and-timers`.

### Requirement: Multiple observers do not implicitly clone a task result
**Reason**: no `Task<T>` handle.
**Migration**: multiple observers use a channel or a deeply immutable shared value — `typed-channels`.

### Requirement: `Task.all`, `Task.first`, and `Task.settled` typing
**Reason**: the `Task.*` combinators are removed.
**Migration**: `concurrent { }` (wait-all), `Concurrent.each` / `Concurrent.of(...).first()` / `.settled()` — `concurrent-blocks-and-timers` and `concurrency-completion`.

## MODIFIED Requirements

### Requirement: Memory and task type family
The type system SHALL define `Weak<T>`, `Pointer<T>`, `NativeSlice<T>`, `NativeSliceMut<T>`, and their capability constraints without exposing mandatory ownership or lifetime parameters. Concurrency handle types (`Job<T>`, `Channel<T>`, `Atomic<T>`, `Mutex<T>`, `Outcome<T>`) are defined by the concurrency-surface changes, not here.

#### Scenario: NativeSlice element type is ABI-safe only
- **WHEN** `NativeSlice<T>`/`NativeSliceMut<T>` is instantiated with an element type outside the ABI-safe subset (`Void`/`Boolean`/fixed-width `Int`/`UInt`/`Float32`/`Float64`/nested `Pointer<T>`)
- **THEN** the checker rejects the instantiation, naming the unsupported element type

#### Scenario: Weak reference type resolves
- **WHEN** `Weak<Node>` is annotated
- **THEN** it resolves as a known type with no ownership parameter

### Requirement: Derived concurrent capabilities
Transferability and shareability SHALL be compiler-derived, non-forgeable properties based on the complete reachable type graph, mutability, resource ownership, and synchronization contract. They SHALL NOT appear in ordinary `Fn` annotations.

#### Scenario: Class contains mutex
- **WHEN** a class safely encapsulates mutable state behind a supported mutex
- **THEN** the compiler may derive sharing without exposing an ordinary user-implemented marker

### Requirement: Concurrency boundary and capture checking

The checker SHALL apply the `Transfer` and `Share` rules at every concurrent-branch creation and capture, at a `concurrent { }` result, and at channel `send` and `receive`. A branch capture of a mutable reference that is neither an exclusive transfer nor statically non-overlapping with the parent's continued use SHALL be rejected with a diagnostic naming transfer, strict immutable sharing, `clone()`, or a channel. A projected capture (`users[0]`) SHALL follow the ordinary projection rule and yield an independent value. A branch SHALL NOT mutate a variable captured from an enclosing scope.

#### Scenario: Ambiguous mutable capture is rejected

- **WHEN** a branch body captures `users` mutably while the parent continues to mutate `users`
- **THEN** compilation fails and lists the resolutions

#### Scenario: Projected capture is independent

- **WHEN** a branch body captures `users[0]` rather than `users`
- **THEN** the child receives the independent projected value and the parent's later list mutations do not affect it
