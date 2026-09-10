## ADDED Requirements

### Requirement: Lowering of concurrent scopes

The lowering SHALL surround a `concurrent { }` block with a `ScopeEnter`
instruction and a `ScopeExit` instruction installed as a cleanup handler on every
exit edge of the block — normal completion, `return`, and exceptional unwind —
reusing the cleanup-edge mechanism used for `finally`, resource close, and
unsafe-journal rollback. `ScopeExit` SHALL run the join-or-cancel-and-clean
protocol before control leaves. The lowering SHALL emit each branch as a boxed
thunk over a garbage-collected capture block and a `BranchStart` instruction,
ordered so a dependent branch's `BranchStart` follows its predecessors' join. A
function body that opens no `concurrent` block SHALL emit no scope instructions.

#### Scenario: scope exit is a cleanup edge on every path
- **WHEN** a `concurrent` block contains an early `return` and can also throw
- **THEN** the `ScopeExit` instruction is reached on the normal path, the `return` path, and the exceptional path

#### Scenario: dependent branch ordering
- **WHEN** a block declares `inmut a = f()` and `inmut b = g(a)`
- **THEN** the IR starts `a`'s branch first and `b`'s `BranchStart` follows `a`'s join, while independent branches start together

### Requirement: Lowering of spawn and job wait

`spawn expr` SHALL lower to a `BranchStart` yielding an `IrType::Job` value
registered with the current scope. `job.wait()` SHALL lower to a `JobWait`
instruction: a suspension point that records the current branch as the job's
waiter and, on resumption, produces the job's result or re-raises its unhandled
failure.

#### Scenario: spawn lowers to a branch start
- **WHEN** `spawn compute()` is lowered
- **THEN** the IR contains a `BranchStart` whose target is the lowered call and whose result is an `IrType::Job`

#### Scenario: job wait is a suspension point
- **WHEN** `h.wait()` is lowered
- **THEN** the IR contains a `JobWait` followed by a resume point yielding the element value

### Requirement: Lowering of Timer operations

`Timer.sleep(d)` SHALL lower to a timer-wait suspension point.
`Timer.after` / `Timer.every` SHALL lower to a `BranchStart` on the nearest
enclosing scope carrying an ambient flag so `ScopeExit` cancels rather than
awaits them.

#### Scenario: Timer.sleep is a suspension point
- **WHEN** `Timer.sleep(2s)` is lowered
- **THEN** the IR contains a timer-wait suspend instruction and a resume label

#### Scenario: ambient timer is cancelled on scope exit
- **WHEN** a `Timer.every` branch is still running when its scope's `ScopeExit` runs
- **THEN** `ScopeExit` cancels it rather than waiting for it
