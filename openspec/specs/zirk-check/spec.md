# zirk-check Specification

## Purpose
Defines the frontend-only `zirk check` command and its diagnostic and artifact guarantees.
## Requirements
### Requirement: Frontend-only validation command

The CLI SHALL expose a `zirk check` subcommand that loads, lexes, parses, resolves names, and performs type and flow checking on one or more `.zrk` source files, stopping before `zirk-ir`, `zirk-codegen-llvm`, or native linking.

The command SHALL report exactly the same diagnostics as `zirk build` for the same input and the same compiler phase, and SHALL NOT produce an executable, object file, or any backend artifact.

#### Scenario: Valid program passes `zirk check`
- **WHEN** a valid `.zrk` entry file is passed to `zirk check`
- **THEN** the command completes the frontend pipeline
- **AND** it exits with code 0
- **AND** no executable or object file is produced

#### Scenario: Invalid program reports a frontend diagnostic
- **WHEN** a `.zrk` file contains a syntax, name-resolution, or type error
- **THEN** the command emits a diagnostic in the same format as `zirk build`
- **AND** the command exits with a non-zero code
- **AND** no IR lowering, object emission, or linking is attempted

#### Scenario: `zirk check` on a nonexistent file
- **WHEN** the path passed to `zirk check` does not exist
- **THEN** the command emits a diagnostic naming the missing path
- **AND** the command exits with a non-zero code

### Requirement: No LLVM dependency

`zirk check` SHALL be buildable and runnable without `zirk-ir`, `zirk-codegen-llvm`, the runtime archive, or a configured LLVM toolchain.

This SHALL be demonstrated by a build that succeeds when `LLVM_SYS_201_PREFIX` is unset and `cargo build -p zirk-cli --bin zirk-check --no-default-features` is executed.

#### Scenario: Build on a machine without LLVM
- **WHEN** `zirk check` is built with the backend features disabled
- **THEN** the build completes without `LLVM_SYS_201_PREFIX`
- **AND** the resulting binary is a functional frontend validator

#### Scenario: Running without `LLVM_SYS_201_PREFIX`
- **WHEN** `zirk check` is executed on a valid `.zrk` file with `LLVM_SYS_201_PREFIX` unset
- **THEN** the command runs and reports the same frontend diagnostics as `zirk build`

### Requirement: Shared diagnostic contract

`zirk check` SHALL use the same diagnostic codes, location formatting, severity rules, and structured JSON output as `zirk build`, so that a consumer parsing the output cannot distinguish which command produced it.

#### Scenario: Same error, same output
- **WHEN** a `.zrk` file is checked with `zirk check` and then built with `zirk build`
- **THEN** both commands emit the same diagnostics for the same line and column
- **AND** both commands exit with the same exit code

#### Scenario: JSON output from `zirk check`
- **WHEN** `zirk check` is run with a structured-output flag
- **THEN** it emits diagnostics in the same JSON schema used by `zirk build`

### Requirement: Exit-code contract

`zirk check` SHALL exit with code 0 when the frontend finds no errors, a non-zero code when any error diagnostic is emitted, and a distinct non-zero code when the command itself is invoked incorrectly.

#### Scenario: Clean check
- **WHEN** the input passes the frontend without errors
- **THEN** `zirk check` exits with code 0

#### Scenario: Check with an error
- **WHEN** the input contains a frontend error
- **THEN** `zirk check` exits with code 1

#### Scenario: Bad invocation
- **WHEN** `zirk check` is called with an invalid argument or no input
- **THEN** `zirk check` exits with code 2


### Requirement: Type-map output mode

`zirk check` SHALL accept a `--types` flag that, alongside diagnostics, emits a structured JSON type map for the checked crate: one entry per declaration name and per resolved expression, carrying the file, the span, the symbol kind (function, method, variable, parameter, field, expression), and the rendered type text. The type map SHALL include inferred types — an unannotated function reports the type the checker inferred — and SHALL distinguish inferred entries from declared ones.

The `--types` mode SHALL remain frontend-only: it produces no executable, object file, or backend artifact, and works on the `zirk-check` binary built without the `backend` feature.

#### Scenario: Type map for an inferred function

- **WHEN** `zirk check --types` runs over `fn hola() { stdout.println("x"); }`
- **THEN** the output contains an entry for `hola` with rendered type `fn(): Void`
- **AND** the entry is marked as inferred

#### Scenario: Type map for a declared variable

- **WHEN** the checked file contains `mut count: Int32 = 0;`
- **THEN** the output contains an entry for `count` with rendered type `Int32`
- **AND** the entry is marked as declared

#### Scenario: Type map on a file with errors

- **WHEN** the checked file contains errors
- **THEN** the type map still reports the entries the checker resolved
- **AND** the diagnostics are emitted as usual

#### Scenario: Type map is frontend-only

- **WHEN** `zirk check --types` runs on a binary built without the `backend` feature
- **THEN** the type map is emitted without error
