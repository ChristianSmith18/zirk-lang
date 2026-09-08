## MODIFIED Requirements

### Requirement: Memory and concurrency implementation order

The roadmap SHALL introduce safe reference and escape foundations before native unsafe APIs, transactional rollback before irreversible commit effects, and structured task semantics before parallelism, OS threads, weak atomics, or advanced synchronization.

Roadmap Phase 5 SHALL be delivered in the sub-steps its narrative names.
Steps 1 to 3 — `Task<T>`, `task`, `await`, `task scope`, sibling-failure
propagation, cooperative cancellation, `cancellation shield`, `await ... timeout`,
`Task.all` / `Task.first` / `Task.settled`, `TaskSettlement<T>`, fair `select`,
and the `Channel<T>` family — SHALL be delivered on a single-threaded cooperative
executor and SHALL leave the garbage collector single-threaded. Within those
steps the runtime infrastructure — the executor, the task control block, the
stackful-coroutine suspend/resume driver, the timer service, per-task
garbage-collection root chains, and the executor lifecycle around `main` — SHALL
be delivered and independently verified before any `task` / `await` / `select`
syntax is parsed, checked, lowered, or emitted. Until that language surface
lands, a program using it SHALL still receive the phase diagnostic. Once
delivered, `task`, `await`, `select`, `cancellation shield`, and the `Task<T>`,
`Channel<T>`, and `TaskSettlement<T>` types are part of the implemented subset and
are no longer deferred constructs. Steps 4 to 6 — `Atomic<T>`, `Mutex<T>` and the
`RwLock<T>` / `Semaphore` / `Barrier` / `Once<T>` library, scoped `thread`,
`task.blocking`, and `parallel` operations and reductions — remain deferred and
SHALL keep emitting the phase diagnostic that names their arrival step.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts task scheduling
- **THEN** typed scopes, cancellation, transfer/share analysis, and cleanup behavior are already specified as prerequisites

#### Scenario: Executor infrastructure precedes the language surface

- **WHEN** the cooperative executor, task control block, timer service, and per-task garbage-collection roots are built
- **THEN** they are verified by runtime tests on their own, and a `.zrk` program that writes `task` or `await` still receives the Phase 5 diagnostic until the structured-tasks change lands

#### Scenario: Structured async core is delivered, not deferred

- **WHEN** a program in the implemented subset uses `task`, `await`, `select`, `cancellation shield`, `Task<T>`, `Channel<T>`, or `TaskSettlement<T>`
- **THEN** no phase diagnostic is emitted and the construct is compiled

#### Scenario: Parallelism and shared-memory synchronization stay deferred

- **WHEN** a program uses `parallel`, `thread`, `task.blocking`, `Mutex`, `RwLock`, `Semaphore`, `Barrier`, `Once`, or `Atomic`
- **THEN** the diagnostic names the construct and the Phase 5 sub-step that delivers it
