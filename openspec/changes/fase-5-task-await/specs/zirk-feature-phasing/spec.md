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
syntax is parsed, checked, lowered, or emitted. The language surface itself SHALL
also arrive in slices: the bare `task expression`, `task { block }`, and
`await expression` forms, with `Task<T>`, are delivered first and independently;
`task scope`, `await ... timeout`, `select`, `cancellation shield`, and the
`Channel<T>` / `TaskSettlement<T>` types SHALL keep emitting the phase diagnostic
until their own change lands. Once a form is delivered it is part of the
implemented subset and is no longer a deferred construct. Steps 4 to 6 —
`Atomic<T>`, `Mutex<T>` and the `RwLock<T>` / `Semaphore` / `Barrier` / `Once<T>`
library, scoped `thread`, `task.blocking`, and `parallel` operations and
reductions — remain deferred and SHALL keep emitting the phase diagnostic that
names their arrival step.

#### Scenario: Phase planning reaches concurrency
- **WHEN** implementation work starts task scheduling
- **THEN** typed scopes, cancellation, transfer/share analysis, and cleanup behavior are already specified as prerequisites

#### Scenario: Executor infrastructure precedes the language surface

- **WHEN** the cooperative executor, task control block, timer service, and per-task garbage-collection roots are built
- **THEN** they are verified by runtime tests on their own, and a `.zrk` program that writes `task` or `await` still receives the Phase 5 diagnostic until the task-await change lands

#### Scenario: The bare task and await forms are delivered first

- **WHEN** a program uses `task expression`, `task { block }`, or `await expression` on a `Task<T>`
- **THEN** no phase diagnostic is emitted and the construct is compiled and run

#### Scenario: The structural async forms stay deferred until their own change

- **WHEN** a program uses `task scope`, `await ... timeout`, `select`, `cancellation shield`, `Channel<T>`, or `TaskSettlement<T>`
- **THEN** the diagnostic names the construct and the Phase 5 slice that delivers it

#### Scenario: Parallelism and shared-memory synchronization stay deferred

- **WHEN** a program uses `parallel`, `thread`, `task.blocking`, `Mutex`, `RwLock`, `Semaphore`, `Barrier`, `Once`, or `Atomic`
- **THEN** the diagnostic names the construct and the Phase 5 sub-step that delivers it
