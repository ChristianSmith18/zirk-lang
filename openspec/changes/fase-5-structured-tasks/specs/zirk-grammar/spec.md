## RENAMED Requirements

- FROM: `### Requirement: Task creation and scope syntax`
- TO: `### Requirement: Task creation syntax`

## MODIFIED Requirements

### Requirement: Constructs outside the subset

The parser SHALL emit a specific diagnostic for constructs that exist in the language but are not yet implemented, distinguishing them from syntax errors.

This phase removes from that list `class`, `construct`, `this`, `record`, `type`, `public`, `private`, `protected`, `abstract`, `implements`, `extends`, `from`, `as`, and `is`.

The structured-async forms `task`, `await`, `final task`, and `select` are also
removed from that list and are parsed as real grammar. `parallel`, `thread`, and
`task.blocking` remain outside the subset and keep receiving the not-implemented
diagnostic. `task scope` and `cancellation shield` are neither implemented nor
deferred: the parser SHALL report them as removed constructs and name their
replacement.

#### Scenario: Construct from a later phase

- **WHEN** `parallel` or `thread` is parsed
- **THEN** the diagnostic SHALL name the construct
- **AND** SHALL indicate that it is not implemented yet
- **AND** SHALL NOT be reported as an unexpected token

#### Scenario: Structured-async form is parsed, not deferred

- **WHEN** `task`, `await`, `final task`, or `select` is parsed
- **THEN** the parser produces the corresponding node and emits no phase diagnostic

#### Scenario: Removed construct is reported as removed

- **WHEN** `task scope` or `cancellation shield` is parsed
- **THEN** the diagnostic states the construct was removed and names its replacement (`Task.combine` / `Task.all`, or `final task { ... }`)

#### Scenario: `init.zrk` out of scope

- **WHEN** an `init.zrk` file is found
- **THEN** the diagnostic indicates that declarative project configuration arrives in a later phase

### Requirement: Task creation syntax

The parser SHALL recognize `task expression` and `task { block }` as expressions
that produce, respectively, a task node over a call and a task node over a block.
The parser SHALL also recognize `final task expression` and `final task { block }`
as the same task nodes carrying a `final` flag. There SHALL be no `task scope`
production; `task` immediately followed by the identifier `scope` SHALL be
reported as a removed construct.

#### Scenario: Task over a call

- **WHEN** source contains `mut u: Task<User> = task load_user(42);`
- **THEN** the parser produces a mutable binding whose initializer is a task node over the call `load_user(42)`

#### Scenario: Final task over a block

- **WHEN** source contains `await final task { tx.rollback_if_open(); };`
- **THEN** the parser produces a task node over the block with the `final` flag set, wrapped in an await node

#### Scenario: `task scope` is a removed construct

- **WHEN** source contains `task scope { ... }`
- **THEN** the parser emits the removed-construct diagnostic and does not treat `scope` as an unexpected token

### Requirement: `final` modifier positions

The parser SHALL accept `final` before `class`, in the member-modifier sequence before a method, and immediately before `task` in an expression position (`final task ...`). `final` in any other position (fields, constructors, `interface`, `trait`, `record`, parameters, variables) SHALL produce a targeted diagnostic.

#### Scenario: Final class and method

- **WHEN** `final class A { }` and `class B { final m(): Void { } }` are parsed
- **THEN** the final flags are recorded on the class and the method

#### Scenario: Final task expression

- **WHEN** `mut h = final task work();` is parsed
- **THEN** the `final` flag is recorded on the task node

#### Scenario: Final field rejected

- **WHEN** a class body contains `final x: Int32;`
- **THEN** a diagnostic is emitted indicating attributes use `inmut`, not `final`

### Requirement: Safety and concurrency grammar

The grammar SHALL parse unsafe function modifiers and blocks, `commit` regions, `task` blocks and callable sugar, `final task` blocks and callable sugar, await timeouts, and `select` branches with `after`, `default`, and cancellation cases without introducing `async fn`. It SHALL NOT provide a `task scope` block or a `cancellation shield` block.

#### Scenario: Select statement is parsed

- **WHEN** source contains task, channel, timer, and default select branches
- **THEN** the parser produces distinct guarded branches and their result bindings

#### Scenario: Final task block is parsed

- **WHEN** source contains `final task { a(); b(); }`
- **THEN** the parser produces a task node over the block with the `final` flag and no `async fn` is involved

## REMOVED Requirements

### Requirement: Cancellation shield syntax

**Reason**: The `cancellation shield` statement is removed from the language. Its
behavior — deferring pending cancellation over a bounded cleanup region — is now
provided by awaiting a `final task { block }`, which is a shielded await.

**Migration**: Replace `cancellation shield { <stmts> }` with
`await final task { <stmts> }`. Multi-step cleanup goes inside the one
`final task` block. Cleanup that performs no `await` needs no wrapper because
nothing interrupts it.
