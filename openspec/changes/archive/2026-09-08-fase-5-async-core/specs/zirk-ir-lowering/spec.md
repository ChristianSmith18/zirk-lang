## ADDED Requirements

### Requirement: Lowering of task creation and awaiting

The lowering SHALL translate `task expression` into an instruction that
allocates a task control block, records the child in the current structured
scope, and starts it, yielding a `Task<T>` value. It SHALL translate `await
handle` into a suspension point: an instruction that suspends the current task,
records the current task as the handle's waiter, and, on resumption, produces the
handle's result value or re-raises its unhandled failure. The lowering SHALL NOT
transform the enclosing function body into a resumable state machine; suspension
SHALL be expressed as a runtime call that saves and restores the running task's
native context.

#### Scenario: Task node lowers to a start instruction

- **WHEN** `task load_user(42)` is lowered
- **THEN** the IR contains a task-start instruction whose operand is the lowered call and whose result is a `Task<T>` value registered with the current scope

#### Scenario: Await lowers to a suspension point

- **WHEN** `await handle` is lowered
- **THEN** the IR contains a suspend instruction followed by a resume point that yields the element value, and the surrounding block structure is otherwise unchanged

### Requirement: Lowering of structured scopes with cleanup edges

The lowering SHALL surround a `task scope { block }` with scope-enter and
scope-exit instructions, and SHALL install the scope-exit as a cleanup handler on
every exit edge of the block — normal completion, `return`, and exceptional
unwind — reusing the cleanup-edge mechanism used for `finally`, resource close,
and unsafe-journal rollback. Scope-exit SHALL run the join-or-cancel-and-clean
protocol before control leaves. A function body that creates no child task SHALL
NOT emit scope-enter or scope-exit instructions.

#### Scenario: Scope exit is a cleanup edge on every path

- **WHEN** a `task scope` block contains an early `return` and can also throw
- **THEN** the scope-exit instruction is reached on the normal path, the `return` path, and the exceptional path

#### Scenario: Non-async function body has no scope overhead

- **WHEN** a function body contains no `task`
- **THEN** its lowered IR contains no scope-enter or scope-exit instruction

### Requirement: Lowering of cancellation, shields, and timeouts

The lowering SHALL translate a cancellation-shield block into an increment of the
running task's shield depth on entry and a decrement on every exit edge, with a
check that delivers any pending cancellation immediately after the closing edge.
It SHALL translate `await expression timeout duration` into arming a timer,
awaiting the operation, and — if the timer fires first — requesting cancellation
of the operation, awaiting its cleanup, and raising `TimeoutError`. A safe point
(`await`, a suspending channel operation, `select`, a timer wait) SHALL lower to
include a cancellation check that raises `CancelledError` when cancellation is
pending and the shield depth is zero.

#### Scenario: Shield defers and then delivers cancellation

- **WHEN** cancellation is requested while the running task is inside a cancellation-shield block
- **THEN** the lowered code completes the block and delivers `CancelledError` at the instruction right after the block

#### Scenario: Timeout cancels and raises

- **WHEN** an awaited operation is still incomplete when its timeout timer fires
- **THEN** the lowered code requests cancellation, awaits cleanup, and raises `TimeoutError` without leaving the operation running

### Requirement: Lowering of `select`

The lowering SHALL evaluate each `select` guard operand once in source order,
register interest on every guard, and then either run the `default` arm when no
guard is ready and a `default` exists, or suspend. On resumption it SHALL choose
one ready guard by a rotating offset for fairness, deregister the other guards
without cancelling their operations, bind the chosen arm's pattern, and run that
arm. Channel closure SHALL be lowered as a ready outcome for a receive guard.

#### Scenario: One guard ready, others preserved

- **WHEN** a `select` resumes with a channel-receive guard ready and a task-completion guard not ready
- **THEN** the receive arm runs and the task-completion guard's operation is left running

#### Scenario: Default runs only when nothing is ready

- **WHEN** a `select` with a `default` arm is entered and no guard is ready
- **THEN** the `default` arm runs and no suspension occurs

### Requirement: Lowering of channel operations

The lowering SHALL translate `channel.send(value)` and `channel.receive()` into
runtime calls that may suspend the current task, and `channel.try_send(value)` /
`channel.try_receive()` into non-suspending runtime calls that return a typed
outcome. A value crossing into or out of a channel SHALL be lowered under the
`Transfer` rule that applies to its type.

#### Scenario: Suspending send lowers to a safe point

- **WHEN** `channel.send(value)` is lowered for a bounded channel
- **THEN** the IR contains a possibly-suspending runtime call and a resume point

#### Scenario: Try-receive lowers without a suspension point

- **WHEN** `channel.try_receive()` is lowered
- **THEN** the IR contains a plain runtime call whose result is a typed outcome and no suspend instruction
