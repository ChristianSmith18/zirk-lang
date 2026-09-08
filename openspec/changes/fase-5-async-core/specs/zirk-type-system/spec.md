## ADDED Requirements

### Requirement: Async core types are known

The checker SHALL recognize `Task<T>`, `Channel<T>`, and the generic enum
`TaskSettlement<T>` with variants `Fulfilled(T)`, `Rejected(Throwable)`, and
`Cancelled(CancelledError)` as ordinary known types, parameterized over any valid
element type. `CancellationReason` SHALL be a known enum whose default value is
`Cancelled`.

#### Scenario: Task annotation resolves

- **WHEN** `mut u: Task<Result<User, LoadError>> = task load_user(42);` is checked
- **THEN** the binding type is `Task<Result<User, LoadError>>` and the initializer must produce that task element type

#### Scenario: Settlement enum is matchable

- **WHEN** a `match` over a `TaskSettlement<User>` value handles `Fulfilled`, `Rejected`, and `Cancelled`
- **THEN** the match is exhaustive

### Requirement: `task` produces a typed handle and `await` produces the element type

The checker SHALL type `task expression` as `Task<T>` where `T` is the type the
expression produces, and SHALL type `task scope { block }` as `Task<T>` where `T`
is the block's result type. `await handle` SHALL be typed as exactly `T` for a
`handle` of type `Task<T>`; `await` SHALL NOT introduce implicit `Result`,
nullability, or exception wrapping. `await expression timeout duration` SHALL
require `duration` to be `Duration` and SHALL be typed as `T`.

#### Scenario: Await unwraps exactly the element type

- **WHEN** `mut r = await someTask;` is checked where `someTask: Task<Int32>`
- **THEN** `r` has type `Int32`

#### Scenario: Timeout operand must be a duration

- **WHEN** `await op timeout 5;` is checked with an integer where a duration is expected
- **THEN** compilation fails naming the required `Duration` type

### Requirement: A task result is consumed exactly once

The checker SHALL treat the result of a `Task<T>` as a linear value: `await` on a
handle SHALL consume it, and a second `await` on the same statically tracked
handle SHALL be a compile-time use-after-consume error that identifies the first
consuming `await`. An ignored `Task<T>` result SHALL be diagnosed under the same
must-use policy as other must-use results, and SHALL be dischargeable with `_ =
handle`, which SHALL NOT detach the task.

#### Scenario: Second await is rejected

- **WHEN** a program awaits a handle and later awaits the same handle again
- **THEN** compilation rejects the second `await` and points at the first

#### Scenario: Explicit discard does not detach

- **WHEN** a program writes `_ = handle;` for a task whose value it does not need
- **THEN** the must-use diagnostic is satisfied and the owning scope still awaits the task's completion and cleanup

### Requirement: Multiple observers do not implicitly clone a task result

The checker SHALL reject an attempt to observe one `Task<T>` result from more than
one place and SHALL direct the developer to `watch`, `broadcast`, a channel, or a
shared deeply immutable value.

#### Scenario: Two consumers of one handle

- **WHEN** two different code paths each `await` the same handle
- **THEN** compilation fails and names the explicit multi-observer mechanisms

### Requirement: Derived Transfer and Share properties

The checker SHALL derive two properties that user code cannot name or forge:
`Transfer` (a value may cross into another concurrent execution context) and
`Share` (one referent may be accessed from more than one concurrent execution
context). These properties SHALL NOT appear in `Fn(...) => R` annotations. Value
types and their projections SHALL be `Transfer` by copy. A complete
`inmut::strict` reference SHALL be both `Transfer` and `Share`. An exclusive
mutable complete reference SHALL be `Transfer` by move, after which the origin
SHALL NOT use it until it returns through a structured result or a channel. A
`clone()` result SHALL be `Transfer` as an independent graph. Task handles,
channel endpoints, pointers, and dependent views SHALL cross only where their own
contracts permit.

#### Scenario: Value crosses by copy

- **WHEN** a task captures an `Int32` local
- **THEN** the child receives an independent copy and the parent keeps its own

#### Scenario: Strict-immutable reference is shared

- **WHEN** a task captures a complete `inmut::strict` reference the parent also reads
- **THEN** the capture is allowed as a shared reference

#### Scenario: Exclusive mutable reference is moved

- **WHEN** a parent passes an exclusive mutable reference into a child task and then reads it
- **THEN** compilation fails because the reference was transferred and has not returned

### Requirement: Concurrency boundary and capture checking

The checker SHALL apply the `Transfer` and `Share` rules at every `task`
creation, `task scope` result, channel `send` and `receive`, and `select` branch
value. A task or `select` capture of a mutable reference that is neither an
exclusive transfer nor statically non-overlapping with the parent's continued use
SHALL be rejected with a diagnostic that names transfer, strict immutable
sharing, `clone()`, or a channel as resolutions. A projected capture (`users[0]`)
SHALL follow the ordinary projection rule and yield an independent value.

#### Scenario: Ambiguous mutable capture is rejected

- **WHEN** a task body captures `users` mutably while the parent continues to mutate `users`
- **THEN** compilation fails and lists the resolutions

#### Scenario: Projected capture is independent

- **WHEN** a task body captures `users[0]` rather than `users`
- **THEN** the child receives the independent projected value and the parent's later list mutations do not affect it

### Requirement: `Task.all`, `Task.first`, and `Task.settled` typing

The checker SHALL type `Task.all(tasks)` over an iterable of `Task<T>` as
producing `List<T>` in input order, `Task.first(tasks)` as producing `T`, and
`Task.settled(tasks)` as producing `List<TaskSettlement<T>>` in input order. A
`Task<Result<U, E>>` input to `Task.settled` SHALL settle as `Fulfilled(Result<U,
E>)`, never as `Rejected` for an ordinary `Error(e)` value.

#### Scenario: All-aggregation result type

- **WHEN** `mut users = await Task.all(taskList);` is checked where `taskList` holds `Task<User>` values
- **THEN** `users` has type `List<User>`

#### Scenario: Settled result type

- **WHEN** `mut settlements = await Task.settled(taskList);` is checked
- **THEN** `settlements` has type `List<TaskSettlement<User>>`
