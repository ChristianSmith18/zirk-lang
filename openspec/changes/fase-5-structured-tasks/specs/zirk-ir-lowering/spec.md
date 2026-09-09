## RENAMED Requirements

- FROM: ### Requirement: Lowering of cancellation, shields, and timeouts
- TO: ### Requirement: Lowering of cancellation and shielded awaits

## MODIFIED Requirements

### Requirement: Lowering of task creation and awaiting

The lowering SHALL translate `task expression` and `final task expression` into a
task-start instruction that allocates a task control block, records the child in
the current structured scope, starts it, and yields a `Task<T>` or `Task.Final<T>`
value; the instruction SHALL carry an `uncancelable` flag set for the `final`
forms. It SHALL translate `await handle` into a suspension point: an instruction
that suspends the current task, records the current task as the handle's waiter,
and, on resumption, produces the handle's result value or re-raises its unhandled
failure. The await instruction SHALL carry a `shielded` flag set when the
handle's static type is `Task.Final<T>`. The lowering SHALL NOT transform the
enclosing function body into a resumable state machine; suspension SHALL be
expressed as a runtime call that saves and restores the running task's native
context.

#### Scenario: Task node lowers to a start instruction

- **WHEN** `task load_user(42)` is lowered
- **THEN** the IR contains a task-start instruction whose operand is the lowered call, whose result is a `Task<T>` value registered with the current scope, and whose `uncancelable` flag is clear

#### Scenario: Final task node sets the uncancelable flag

- **WHEN** `final task cleanup()` is lowered
- **THEN** the IR contains a task-start instruction whose `uncancelable` flag is set and whose result is a `Task.Final<T>` value

#### Scenario: Await lowers to a suspension point

- **WHEN** `await handle` is lowered where `handle: Task<T>`
- **THEN** the IR contains a suspend instruction with a clear `shielded` flag followed by a resume point that yields the element value

#### Scenario: Await of a final handle sets the shielded flag

- **WHEN** `await handle` is lowered where `handle: Task.Final<T>`
- **THEN** the suspend instruction's `shielded` flag is set

### Requirement: Lowering of structured scopes with cleanup edges

The lowering SHALL surround a structured scope — a function body or a `task { block }`
that creates at least one child task — with scope-enter and scope-exit
instructions, and SHALL install the scope-exit as a cleanup handler on every exit
edge — normal completion, `return`, and exceptional unwind — reusing the
cleanup-edge mechanism used for `finally`, resource close, and unsafe-journal
rollback. Scope-exit SHALL run the join-or-cancel-and-clean protocol before
control leaves. A function body that creates no child task SHALL NOT emit
scope-enter or scope-exit instructions. There SHALL be no `task scope` block to
lower.

#### Scenario: Scope exit is a cleanup edge on every path

- **WHEN** a function body spawns a child task and contains an early `return` and can also throw
- **THEN** the scope-exit instruction is reached on the normal path, the `return` path, and the exceptional path

#### Scenario: Non-async function body has no scope overhead

- **WHEN** a function body contains no `task`
- **THEN** its lowered IR contains no scope-enter or scope-exit instruction

### Requirement: Lowering of cancellation and shielded awaits

The lowering SHALL translate a `shielded` await into an increment of the running
task's shield depth before the suspend instruction and a decrement on every
resume or unwind edge, with a check that delivers any pending cancellation
immediately after the shield depth returns to its prior value. It SHALL translate
`await expression timeout duration` into arming a timer, awaiting the operation,
and — if the timer fires first — requesting cancellation of the operation,
awaiting its cleanup, and raising `TimeoutError`. A safe point (`await`, a
suspending channel operation, `select`, a timer wait) SHALL lower to include a
cancellation check that raises `CancelledError` when cancellation is pending and
the shield depth is zero. A task-start instruction with the `uncancelable` flag
SHALL lower so the runtime marks the child's control block uncancelable.

#### Scenario: Shielded await defers and then delivers cancellation

- **WHEN** cancellation is requested while the running task is inside a shielded await of a `final task` block
- **THEN** the lowered code completes the awaited block and delivers `CancelledError` at the instruction right after the await

#### Scenario: Timeout cancels and raises

- **WHEN** an awaited operation is still incomplete when its timeout timer fires
- **THEN** the lowered code requests cancellation, awaits cleanup, and raises `TimeoutError` without leaving the operation running

#### Scenario: Uncancelable child is marked at start

- **WHEN** a `final task` start instruction is lowered
- **THEN** the emitted runtime call marks the new control block uncancelable
