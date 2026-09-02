## ADDED Requirements

### Requirement: Translation from IR to LLVM

The backend SHALL translate the typed IR to LLVM IR, preserving type semantics and control flow.

#### Scenario: Verifiable module
- **WHEN** a well-formed IR is translated
- **THEN** the resulting LLVM module passes LLVM's verification

#### Scenario: Block correspondence
- **WHEN** a function with a conditional is translated
- **THEN** the IR's basic blocks correspond to LLVM basic blocks

### Requirement: Controlled arithmetic overflow

Arithmetic operations on integers SHALL detect overflow and produce a controlled runtime error, not silent wraparound.

This is required by `ZIRK_LANGUAGE_SPEC.md` section 3: the wrapping, saturating, or checked variants must be explicit operations, which do not exist in this subset.

#### Scenario: Overflowing addition
- **WHEN** an `Int32` addition exceeds the type's range at runtime
- **THEN** the program terminates with a diagnosed runtime error
- **AND** it does NOT produce a wrapped result

#### Scenario: Division by zero
- **WHEN** a division by zero occurs at runtime
- **THEN** the program terminates with a diagnosed runtime error
- **AND** it does NOT incur undefined behavior

### Requirement: Executable linking

The pipeline SHALL produce a native executable by linking the generated object with the Zirk runtime.

#### Scenario: Executable produced
- **WHEN** a valid program from the subset is compiled
- **THEN** an executable for the host platform is produced
- **AND** the executable links the runtime's static library

#### Scenario: Link failure
- **WHEN** the linker fails
- **THEN** a diagnostic including the linker's output is emitted
- **AND** the help indicates how to verify the toolchain

### Requirement: Lifecycle of the generated program

The generated code SHALL invoke runtime initialization before the body of `main` and its shutdown afterward, per `ZIRK_RUNTIME_SPEC.md` section 2.

#### Scenario: Invocation order
- **WHEN** the generated entrypoint is inspected
- **THEN** it invokes `zirk_rt_init` before the body of `main`
- **AND** it invokes `zirk_rt_shutdown` after `main` returns

#### Scenario: Exit code
- **WHEN** a program from the subset terminates normally
- **THEN** the process exits with exit code 0
