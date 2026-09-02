## REMOVED Requirements

### Requirement: Absence of Zirk syntax at this phase

**Reason**: This was a Phase 0-specific restriction, whose goal was to assemble the foundations without implementing any language. This phase implements exactly the lexical, syntactic, and semantic analysis that requirement prohibited.

**Migration**: It is replaced by the *Pipeline coverage per crate* requirement, which fixes which stage lives in which crate instead of requiring them to be empty.

## ADDED Requirements

### Requirement: Pipeline coverage per crate

Each stage of the pipeline from `ZIRK_COMPILER_SPEC.md` section 2 SHALL be implemented in its corresponding crate, with no stage taking on another's responsibilities.

#### Scenario: The lexer doesn't know the grammar
- **WHEN** `zirk-lexer` is inspected
- **THEN** it produces tokens
- **AND** it does NOT decide whether a sequence of tokens is valid

#### Scenario: The parser doesn't check types
- **WHEN** an expression with incompatible types but correct syntax is parsed
- **THEN** the parser produces the tree without error
- **AND** the type checker emits the type error

#### Scenario: The backend is the only one that knows LLVM
- **WHEN** the workspace crates are inspected
- **THEN** only `zirk-codegen-llvm` depends on `inkwell`

#### Scenario: Linking does not happen in the backend
- **WHEN** an executable is produced
- **THEN** the backend emits the object
- **AND** the linker invocation happens in the CLI
