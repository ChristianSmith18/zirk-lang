# zirk-feature-phasing

## Purpose

Defines the discipline that keeps the compiler honest about what it does not implement yet.

The language is documented in full while it is built in phases. That gap is deliberate, and it is only safe as long as every documented feature has an owning phase and the compiler names that phase instead of failing as if the feature did not exist.
## Requirements
### Requirement: Every documented feature has an owning phase

Every feature defined by the normative sources SHALL have exactly one
construction phase assigned to it in `docs/init/ZIRK_ROADMAP.md`.

A feature without an assigned phase is not a deferred feature: it is a
feature that nobody will build. Assigning it is what prevents it from
sneaking into the active phase and overflowing it, or from remaining as debt
with no due date.

The infix exponentiation operator `**` and its compound form `**=` are
delivered by Phase 3b, together with the numeric families they operate on.
Once delivered, `**` is part of the implemented subset and is no longer a
deferred construct.

#### Scenario: Normative feature without a phase
- **WHEN** a normative source defines a feature that no phase of the roadmap names
- **THEN** the roadmap is corrected by assigning it a phase before implementing any part of it

#### Scenario: Features orphaned by normative refinement
- **WHEN** the `Float` family, graphemic `Char`, deep contextual conversion, the bitwise and shift operators, and string interpolation are consulted
- **THEN** the roadmap assigns them to Phase 3b
- **AND** assigns `inmut::strict` to Phase 4 and the temporal family to Phase 7

#### Scenario: Exponentiation operator is delivered, not deferred
- **WHEN** a program in the implemented subset uses `**` or `**=`
- **THEN** no phase diagnostic is emitted and the operator is compiled

### Requirement: Phase diagnostic for what is not implemented

The compiler SHALL emit a diagnostic naming the construct and its arrival
phase for any construct, keyword, operator, literal, or type that belongs to
the language but not to the implemented phase.

The compiler SHALL NOT produce a generic syntax error, an "unrecognized
character" error, or a silent interpretation different from the construct as
written.

#### Scenario: Construct from a later phase
- **WHEN** the source contains a language construct that the current phase does not implement
- **THEN** the diagnostic names it and indicates the phase in which it arrives

#### Scenario: Absence of silent interpretation
- **WHEN** the source contains a literal or an operator of the language that the current phase does not implement
- **THEN** the compiler recognizes it as such and defers it along with its phase
- **AND** it does NOT reinterpret it as a different sequence of tokens

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
