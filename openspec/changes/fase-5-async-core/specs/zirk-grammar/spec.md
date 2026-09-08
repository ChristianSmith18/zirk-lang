## MODIFIED Requirements

### Requirement: Constructs outside the subset

The parser SHALL emit a specific diagnostic for constructs that exist in the language but are not yet implemented, distinguishing them from syntax errors.

This phase removes from that list `class`, `construct`, `this`, `record`, `type`, `public`, `private`, `protected`, `abstract`, `implements`, `extends`, `from`, `as`, and `is`.

The structured-async forms `task`, `await`, `task scope`, `select`, and
`cancellation shield` are also removed from that list and are parsed as real
grammar. `parallel`, `thread`, and `task.blocking` remain outside the subset and
keep receiving the diagnostic.

#### Scenario: Construct from a later phase

- **WHEN** `parallel` or `thread` is parsed
- **THEN** the diagnostic SHALL name the construct
- **AND** SHALL indicate that it is not implemented yet
- **AND** SHALL NOT be reported as an unexpected token

#### Scenario: Structured-async form is parsed, not deferred

- **WHEN** `task`, `await`, `task scope`, `select`, or `cancellation shield` is parsed
- **THEN** the parser produces the corresponding node and emits no phase diagnostic

#### Scenario: `init.zrk` out of scope

- **WHEN** an `init.zrk` file is found
- **THEN** the diagnostic indicates that declarative project configuration arrives in a later phase

## ADDED Requirements

### Requirement: Task creation and scope syntax

The parser SHALL recognize `task expression`, `task { block }`, and `task scope {
block }` as expressions that produce, respectively, a task node over a call, a
task node over a block, and a structured-scope node over a block. `task scope`
SHALL be usable where an expression is expected and its value SHALL be the block's
value. A control transfer that would leave a `task scope` block other than
`return` or normal completion SHALL be rejected with a diagnostic.

#### Scenario: Task over a call

- **WHEN** source contains `mut u: Task<User> = task load_user(42);`
- **THEN** the parser produces a mutable binding whose initializer is a task node over the call `load_user(42)`

#### Scenario: Task scope as an expression

- **WHEN** source contains `mut dashboard = task scope { mut a = task load_a(); return combine(await a); };`
- **THEN** the parser produces a structured-scope node whose value is the block result

#### Scenario: Illegal jump out of a task scope

- **WHEN** a `break` targeting an outer loop appears directly inside a `task scope` block
- **THEN** compilation fails and identifies the crossed `task scope` boundary

### Requirement: Await and timeout syntax

The parser SHALL recognize `await expression` as an expression and `await
expression timeout duration` as an expression whose `timeout` operand is a
duration-typed expression. `timeout` in this position SHALL be a contextual
keyword and SHALL remain usable as an identifier elsewhere.

#### Scenario: Plain await

- **WHEN** source contains `mut r = await handle;`
- **THEN** the parser produces an await node over `handle` with no timeout operand

#### Scenario: Await with a timeout

- **WHEN** source contains `mut r = await operation timeout 5s;`
- **THEN** the parser produces an await node whose timeout operand is the duration expression `5s`

#### Scenario: `timeout` as an identifier

- **WHEN** source declares `mut timeout: Int32 = 30;` outside an await expression
- **THEN** the parser accepts `timeout` as an ordinary identifier

### Requirement: Select syntax

The parser SHALL recognize `select { arm, ... }` where each arm is a guard
followed by `=>` and a branch expression or block. A guard SHALL be one of a
binding `pattern = await channel-or-handle operation`, `after duration`,
`cancelled`, or `default`. At most one `default` arm SHALL be allowed. `select`,
`after`, and `cancelled` SHALL be contextual in this position.

#### Scenario: Select with mixed guards

- **WHEN** source contains a `select` with a channel-receive arm, an `after 5s` arm, and a `cancelled` arm
- **THEN** the parser produces a select node with three guarded arms and no default

#### Scenario: Select with a default arm

- **WHEN** a `select` contains a `default =>` arm
- **THEN** the parser records it as the non-suspending fallback arm

#### Scenario: Two default arms are rejected

- **WHEN** a `select` contains two `default =>` arms
- **THEN** compilation fails with a duplicate-default diagnostic

### Requirement: Cancellation shield syntax

The parser SHALL recognize `cancellation shield { block }` as a statement whose
block is a bounded non-interruptible region. `shield` SHALL be contextual after
`cancellation`.

#### Scenario: Shielded cleanup block

- **WHEN** source contains `cancellation shield { await persist_commit(); }`
- **THEN** the parser produces a cancellation-shield node wrapping the block
