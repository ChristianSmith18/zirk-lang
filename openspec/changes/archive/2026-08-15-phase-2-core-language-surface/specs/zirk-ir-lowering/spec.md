## MODIFIED Requirements

### Requirement: Basic block shape

The IR SHALL represent the body of each function as a graph of basic blocks, each terminated by exactly one termination instruction. The graph SHALL admit cycles, produced by loop lowering.

This is what enables the flow analysis `ZIRK_COMPILER_SPEC.md` section 4 requires, and the shape Phase 1 (ADR-007) already anticipated with this phase's loops in mind.

#### Scenario: Block with a single terminator
- **WHEN** any basic block is inspected
- **THEN** it ends in a jump, conditional jump, or return
- **AND** it contains no termination instructions in an intermediate position

#### Scenario: Conditional
- **WHEN** an `if` statement with both branches is lowered
- **THEN** blocks are produced for the condition, each branch, and the continuation
- **AND** the condition block ends in a conditional jump

#### Scenario: Loop
- **WHEN** a `while`, `loop`, or `for` is lowered
- **THEN** a condition or body block that jumps back to itself or to a previous block is produced
- **AND** the resulting block graph contains a cycle

### Requirement: Independence from the memory model

The IR SHALL NOT express operations tied to a concrete memory strategy. Allocation is expressed abstractly and resolved by the runtime. This includes allocation of a closure's capture environment.

This is a direct requirement of `docs/decisions/ADR-003-memoria.md`.

#### Scenario: Allocation operation
- **WHEN** the IR needs to express that a value is allocated
- **THEN** it uses an abstract operation that names the type
- **AND** it does NOT name `malloc`, reference counting, or garbage collection

#### Scenario: Allocation of a closure's environment
- **WHEN** a lambda that captures variables from the enclosing scope is lowered
- **THEN** the capture environment is allocated with the same abstract operation as any other type
- **AND** the IR does not name where that environment lives in memory

## ADDED Requirements

### Requirement: Lowering of loops and of `break`/`continue`

Lowering SHALL translate `for`, `for ... in`, `while`, and `loop` into basic blocks with the condition evaluated in its own block, and SHALL translate `break`/`continue` into a direct jump to the continuation block or to the condition block of the loop that contains them.

#### Scenario: `while`
- **WHEN** `while cond { body }` is lowered
- **THEN** a condition block, a body block, and a continuation block are produced
- **AND** the body block ends by jumping back to the condition block

#### Scenario: `break`
- **WHEN** a `break` inside a loop is lowered
- **THEN** a direct jump to that loop's continuation block is produced

#### Scenario: `continue`
- **WHEN** a `continue` inside a `for` is lowered
- **THEN** a direct jump to the `for`'s increment block is produced, not to the condition block

#### Scenario: Nested loops
- **WHEN** a `break` is inside an inner loop, nested within an outer one
- **THEN** the produced jump points to the inner loop's continuation, not the outer loop's

### Requirement: Lowering of `if` as an expression

Lowering SHALL translate an `if`/`else` in expression position into blocks whose continuation block receives the value of the taken branch, without introducing an instruction form different from the one the `if` statement already uses.

#### Scenario: Value of the selected branch
- **WHEN** `mut r = if x > 0 { a } else { b };` is lowered
- **THEN** the continuation block produces a value that comes from the block of the branch that executed

### Requirement: Closure lowering

Lowering SHALL translate a lambda into an independent function plus an environment value with the captured values copied at the point of creation, and SHALL translate a call to a closure as an indirect call that receives the environment as an implicit argument.

#### Scenario: Creating a closure
- **WHEN** `inmut ADD = (a: Int32, b: Int32): Int32 => a + b;` is lowered with no captures
- **THEN** an independent function and a closure value with no environment, or with an empty environment, are produced

#### Scenario: Closure with a capture
- **WHEN** a lambda references a variable from the enclosing scope
- **THEN** the allocated environment contains a copy of that variable at the moment of creation
- **AND** the lowered function's body reads the variable from the environment, not from the original slot

### Requirement: `match` lowering

Lowering SHALL translate a `match` into a sequence of comparisons over the `enum`'s discriminant (or over the value, for literals), each with a conditional jump to its arm's block, ending at the `_` wildcard's block if it exists.

#### Scenario: `match` over an `enum`
- **WHEN** a `match` with one arm per constructor is lowered
- **THEN** one comparison block per constructor and one block per arm body are produced

#### Scenario: `match` as an expression
- **WHEN** a `match` used as an expression is lowered
- **THEN** each arm block ends by jumping to a common continuation block that receives that arm's value

### Requirement: Lowering of null coalescing

Lowering SHALL translate `a ?? b` into an explicit null check with two blocks, which produces `a` when it is not null and evaluates `b` in the other block.

#### Scenario: Check with two blocks
- **WHEN** `a ?? b` is lowered
- **THEN** a null check over `a` is produced, with one block per outcome

#### Scenario: Null coalescing evaluates the fallback lazily
- **WHEN** `a ?? costoso()` is lowered
- **THEN** `costoso()` is only lowered inside the block that executes when `a` is null
