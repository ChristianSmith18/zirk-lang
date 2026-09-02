## ADDED Requirements

### Requirement: Compiling a file

The CLI SHALL expose a subcommand that compiles a `.zrk` file and produces a native executable.

#### Scenario: Successful compilation
- **WHEN** a valid file from the subset is compiled
- **THEN** an executable is produced
- **AND** the CLI exits with exit code 0

#### Scenario: Compilation with errors
- **WHEN** the file contains errors
- **THEN** the corresponding diagnostics are emitted
- **AND** no executable is produced
- **AND** the CLI exits with a code other than 0

#### Scenario: Nonexistent file
- **WHEN** a path that does not exist is given
- **THEN** a diagnostic naming the path is emitted

### Requirement: Compile and run

The CLI SHALL expose a subcommand that compiles and runs the program in a single step.

#### Scenario: Execution after compiling
- **WHEN** a valid program from the subset is run
- **THEN** the program runs and its output appears on standard output

#### Scenario: Exit code propagation
- **WHEN** the compiled program terminates with an exit code
- **THEN** the CLI exits with that same code

### Requirement: Diagnostic presentation

The CLI SHALL present diagnostics in the format from `ZIRK_COMPILER_SPEC.md` section 8, and offer structured output for tooling.

#### Scenario: Human-readable format
- **WHEN** a diagnostic is emitted without requesting structured output
- **THEN** it is printed with severity, code, location, source excerpt, cause, and help

#### Scenario: Structured output
- **WHEN** structured output is requested
- **THEN** diagnostics are emitted in a machine-readable format

#### Scenario: Diagnostics go to stderr
- **WHEN** diagnostics are emitted
- **THEN** they are written to standard error
- **AND** they are NOT mixed with the compiled program's output

### Requirement: Single-file scope

The CLI at this phase SHALL operate on a single file, with no project manifest.

#### Scenario: Absence of a manifest
- **WHEN** a file is compiled with no `init.zrk` present
- **THEN** compilation proceeds normally

#### Scenario: Multiple files
- **WHEN** several source files are given
- **THEN** a diagnostic is emitted indicating that multi-file projects arrive in a later phase
