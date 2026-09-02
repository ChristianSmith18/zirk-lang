## MODIFIED Requirements

### Requirement: IR-to-LLVM translation

The backend SHALL translate the typed IR to LLVM IR, preserving type semantics and control flow, including block graphs with cycles.

#### Scenario: Verifiable module
- **WHEN** a well-formed IR is translated
- **THEN** the resulting LLVM module passes LLVM verification

#### Scenario: Block correspondence
- **WHEN** a function with a conditional is translated
- **THEN** the IR's basic blocks correspond to LLVM basic blocks

#### Scenario: Correspondence of a cycle
- **WHEN** a function with a loop is translated
- **THEN** the LLVM block for the loop body jumps back to the condition block
- **AND** the resulting module passes LLVM verification

## ADDED Requirements

### Requirement: Closure codegen

The backend SHALL translate a closure into an independent LLVM function that receives the environment as an implicit first argument, plus an aggregate value (function pointer, environment pointer) for use as a first-class value.

#### Scenario: Direct call to a closure
- **WHEN** a call to a closure value is translated
- **THEN** the generated code extracts the function pointer and the environment pointer from the aggregate value
- **AND** it invokes the function with the environment as its first argument

#### Scenario: Closure without captures
- **WHEN** a lambda that captures no variable is translated
- **THEN** the generated environment occupies no space observable to the program

### Requirement: `match` codegen

The backend SHALL translate the `match` lowered by the IR to a sequence of comparisons and conditional jumps over the discriminant, or to an LLVM `switch` instruction when the `match` is exhaustive over an `enum`.

#### Scenario: `match` over an exhaustive `enum`
- **WHEN** a `match` that covers every constructor of an `enum` is translated
- **THEN** the generated code uses an LLVM `switch` over the discriminant
- **AND** it does not include a default branch observable at runtime when there is no `_`

### Requirement: Null check

The backend SHALL translate the explicit null check produced by `??`'s lowering into a comparison against the null value of the type's representation, at no additional cost for values type checking already proved non-null.

#### Scenario: Translated coalescing
- **WHEN** `name ?? "anonymous"` is translated
- **THEN** the generated code compares the value of `nombre` against null before choosing the branch

#### Scenario: No check when the type is not nullable
- **WHEN** an operation over a value of a non-nullable type is translated
- **THEN** the generated code includes no null comparison
