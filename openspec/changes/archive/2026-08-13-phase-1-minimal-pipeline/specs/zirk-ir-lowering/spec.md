## ADDED Requirements

### Requirement: Typed IR

The intermediate representation SHALL be typed: every operation and every value knows its type, per `ZIRK_COMPILER_SPEC.md` section 4.

#### Scenario: Type of every value
- **WHEN** any IR value is inspected
- **THEN** it exposes its type

#### Scenario: Operation with incompatible operands
- **WHEN** an operation whose operands do not match its signature is constructed
- **THEN** construction fails and does NOT produce invalid IR

### Requirement: Basic-block form

The IR SHALL represent each function's body as a graph of basic blocks, each ending with exactly one terminating instruction.

This is what enables the flow analysis that `ZIRK_COMPILER_SPEC.md` section 4 requires, and what avoids reworking the representation when loops arrive in Phase 2.

#### Scenario: Block with a single terminator
- **WHEN** any basic block is inspected
- **THEN** it ends in a jump, conditional jump, or return
- **AND** it contains no terminating instructions in an intermediate position

#### Scenario: Conditional
- **WHEN** an `if` statement with both branches is lowered
- **THEN** blocks are produced for the condition, each branch, and the continuation
- **AND** the condition block ends in a conditional jump

### Requirement: Independence from the memory model

The IR SHALL NOT express operations tied to a concrete memory strategy. Allocation is expressed abstractly and resolved by the runtime.

This is a direct requirement from `docs/decisions/ADR-003-memoria.md`: the strategy is decided in Phase 4 and the IR must not anticipate it.

#### Scenario: Allocation operation
- **WHEN** the IR needs to express that a value is allocated
- **THEN** it uses an abstract operation that names the type
- **AND** it does NOT name `malloc`, reference counting, or garbage collection

### Requirement: Local variables as slots

Local variables SHALL be represented as slots with load and store operations, with no custom SSA form.

Promotion to registers is delegated to the backend. A hand-rolled SSA does not pay off until custom optimizations exist.

#### Scenario: Reading and writing a local
- **WHEN** the use of a local variable is lowered
- **THEN** a load from its slot is produced
- **AND** an assignment produces a store into that slot

### Requirement: Traceability to the source

Every IR instruction SHALL retain the location in the source that originated it.

Without it, the faithful mapping to `.zrk` that `ZIRK_COMPILER_SPEC.md` section 11 requires from the debugger is not possible.

#### Scenario: Location of an instruction
- **WHEN** any IR instruction is inspected
- **THEN** it exposes the corresponding source location

### Requirement: Lowering from the verified tree

IR generation SHALL start from the already resolved and verified tree, not from the parser's raw tree.

#### Scenario: Input to lowering
- **WHEN** IR is generated
- **THEN** the input has resolved names and verified types
- **AND** lowering does NOT check types again
