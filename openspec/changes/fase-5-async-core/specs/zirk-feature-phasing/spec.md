## MODIFIED Requirements

### Requirement: Memory and concurrency implementation order

The roadmap SHALL introduce safe reference and escape foundations before native unsafe APIs, transactional rollback before irreversible commit effects, and structured task semantics before parallelism, OS threads, weak atomics, or advanced synchronization.

Roadmap Phase 5 SHALL be delivered in the sub-steps its narrative names.
Steps 1 to 3 — `Task<T>`, `task`, `await`, `task scope`, sibling-failure
propagation, cooperative cancellation, `cancellation shield`, `await ... timeout`,
`Task.all` / `Task.first` / `Task.settled`, `TaskSettlement<T>`, fair `select`,
and the `Channel<T>` family — SHALL be delivered on a single-threaded cooperative
executor and SHALL leave the garbage collector single-threaded. Once delivered,
`task`, `await`, `select`, `cancellation shield`, and the `Task<T>`,
`Channel<T>`, and `TaskSettlement<T>` types are part of the implemented subset and
are no longer deferred constructs. Steps 4 to 6 — `Atomic<T>`, `Mutex<T>` and the
`RwLock<T>` / `Semaphore` / `Barrier` / `Once<T>` library, scoped `thread`,
`task.blocking`, and `parallel` operations and reductions — remain deferred and
SHALL keep emitting the phase diagnostic that names their arrival step.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts task scheduling
- **THEN** typed scopes, cancellation, transfer/share analysis, and cleanup behavior are already specified as prerequisites

#### Scenario: Structured async core is delivered, not deferred

- **WHEN** a program in the implemented subset uses `task`, `await`, `select`, `cancellation shield`, `Task<T>`, `Channel<T>`, or `TaskSettlement<T>`
- **THEN** no phase diagnostic is emitted and the construct is compiled

#### Scenario: Parallelism and shared-memory synchronization stay deferred

- **WHEN** a program uses `parallel`, `thread`, `task.blocking`, `Mutex`, `RwLock`, `Semaphore`, `Barrier`, `Once`, or `Atomic`
- **THEN** the diagnostic names the construct and the Phase 5 sub-step that delivers it

### Requirement: The table of pending types reflects the current language

The table of known-but-not-implemented types SHALL contain only types that
the language defines today, each with the phase that actually brings it.

A type withdrawn from the language SHALL disappear from the table:
announcing its arrival would teach a language that does not exist.

`Task<T>`, `Channel<T>`, and `TaskSettlement<T>` SHALL NOT appear in the table
once Phase 5 steps 1 to 3 are delivered; they resolve as ordinary known types.
`Atomic<T>` SHALL remain in the table with its Phase 5 arrival step.

#### Scenario: Type withdrawn from the language
- **WHEN** an annotation names `Decimal64` or another member of the old `Decimal` family
- **THEN** the diagnostic treats it as a nonexistent type
- **AND** it does NOT announce any arrival phase

#### Scenario: Correctly declared phase
- **WHEN** an annotation names a pending type
- **THEN** the phase indicated by the diagnostic matches the one the roadmap assigns to it

#### Scenario: Delivered async type no longer pends

- **WHEN** an annotation names `Channel<Int32>` or `Task<String>` after Phase 5 steps 1 to 3
- **THEN** it resolves as a known type and no pending-type diagnostic is emitted

#### Scenario: Atomic still pends

- **WHEN** an annotation names `Atomic<Int32>`
- **THEN** the pending-type diagnostic names it and its Phase 5 arrival step
