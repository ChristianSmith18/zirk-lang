# zirk-cli-commands

## Purpose

Defines the commands that compile and run a program, and how diagnostics are presented.
## Requirements
### Requirement: Compiling a file

The CLI SHALL expose a subcommand that compiles a source file and produces a native executable. The source argument MAY be given with or without the `.zrk` extension; when it is omitted, the CLI SHALL append `.zrk` before reading the file.

#### Scenario: Successful compilation with extension
- **WHEN** a valid file from the subset is compiled using the `.zrk` extension
- **THEN** an executable is produced
- **AND** the CLI exits with code 0

#### Scenario: Successful compilation without extension
- **WHEN** a valid file from the subset is compiled without the `.zrk` extension
- **THEN** an executable is produced
- **AND** the CLI exits with code 0

#### Scenario: Compilation with errors
- **WHEN** the file contains errors
- **THEN** the corresponding diagnostics are emitted
- **AND** no executable is produced
- **AND** the CLI exits with a non-zero exit code

#### Scenario: Nonexistent file
- **WHEN** a path that does not exist is given
- **THEN** a diagnostic is emitted naming the resolved path

### Requirement: Compile and run

The CLI SHALL expose a subcommand that compiles and runs the program in a single step. The source argument MAY be given with or without the `.zrk` extension.

#### Scenario: Execution after compiling with extension
- **WHEN** a valid program from the subset is run using the `.zrk` extension
- **THEN** the program runs and its output appears on standard output

#### Scenario: Execution after compiling without extension
- **WHEN** a valid program from the subset is run without the `.zrk` extension
- **THEN** the program runs and its output appears on standard output

#### Scenario: Exit code propagation
- **WHEN** the compiled program exits with an exit code
- **THEN** the CLI exits with that same code

### Requirement: Diagnostics presentation

The CLI SHALL present diagnostics in the format defined by `ZIRK_COMPILER_SPEC.md` section 8, and offer structured output for tools.

#### Scenario: Human-readable format
- **WHEN** a diagnostic is emitted without requesting structured output
- **THEN** it is printed with severity, code, location, source snippet, cause, and help

#### Scenario: Structured output
- **WHEN** structured output is requested
- **THEN** diagnostics are emitted in machine-readable format

#### Scenario: Diagnostics go to the error output
- **WHEN** diagnostics are emitted
- **THEN** they are written to standard error
- **AND** they are NOT mixed with the compiled program's output

### Requirement: Single-file scope

The CLI in this phase SHALL operate on a single file, with no project manifest.

#### Scenario: Absence of a manifest
- **WHEN** a file is compiled without an `init.zrk` present
- **THEN** compilation proceeds normally

#### Scenario: Multiple files
- **WHEN** several source files are given
- **THEN** a diagnostic is emitted indicating that multi-file projects arrive in a later phase

### Requirement: Authority-bearing commands validate signed approval incrementally
Before build-time or runtime code executes, relevant CLI commands SHALL compare project name/location, manifest, lockfile, permission, requester, phase, and approval fingerprints. They SHALL take a fast path when unchanged and recompute only affected requester graph segments when changed.

#### Scenario: Dependency requester updated
- **WHEN** an approved dependency receiving filesystem authority changes version or integrity
- **THEN** the CLI stops and requests new approval even when textual permission scope is unchanged

### Requirement: Permission management commands are auditable
The CLI SHALL provide `zirk permissions show`, `diff`, `approve`, `revoke`, and `history`. Interactive approval SHALL show exact scope, phase, call/requester path, and manifest diff. CI SHALL consume a protected explicit policy and fail with a diff when authority widens.

#### Scenario: Noninteractive build lacks approval
- **WHEN** CI encounters a permission fingerprint absent from its protected policy
- **THEN** the command fails without prompting and prints a machine-readable authorization diff

### Requirement: Test selection and reporting are reproducible
The CLI MUST provide `zirk test` unit, E2E, all, file, tag, seed, and bounded-job
selection together with human, JSON, and JUnit reports. Filtering SHALL NOT
change test semantics or grant permissions. Every failure affected by
runner-controlled randomness SHALL report a reproduction seed; that seed SHALL
NOT control cryptographic randomness.

#### Scenario: Seeded test fails
- **WHEN** a test using runner-controlled randomization fails under `--seed 48291`
- **THEN** the report includes `48291` and a reproducible command

#### Scenario: CI requests JUnit output
- **WHEN** `zirk test --report junit` is executed
- **THEN** the runner emits stable interoperable test records with the same secret redaction as human output

### Requirement: Snapshot updates are explicit
The test runner SHALL compare snapshots without rewriting them by default.
Snapshot writes MUST require `--update-snapshots`, show changes, enforce test
filesystem permission, and preserve secret redaction.

#### Scenario: Snapshot differs in a normal test run
- **WHEN** an observed snapshot differs without the update flag
- **THEN** the test fails with a diff
- **AND** the stored snapshot remains unchanged

### Requirement: Check subcommand in the CLI

The CLI SHALL expose `zirk check` as a first-class subcommand, alongside `zirk build` and `zirk run`. The source argument for `zirk check` MAY be given with or without the `.zrk` extension.

#### Scenario: Help lists the check subcommand
- **WHEN** `zirk --help` or `zirk check --help` is requested
- **THEN** `check` appears in the list of subcommands
- **AND** its description states that it runs the frontend without producing a native binary

#### Scenario: Running `zirk check` with an entry file
- **WHEN** `zirk check <path>` is invoked
- **THEN** the `zirk check` binary runs the frontend on the resolved path
- **AND** the command returns the same exit code as if the `zirk-check` binary were invoked directly

#### Scenario: Running `zirk check` without the `.zrk` extension
- **WHEN** `zirk check <path-without-extension>` is invoked
- **THEN** the CLI resolves `<path-without-extension>.zrk` and runs the frontend on it
- **AND** the command returns the same exit code as if the `.zrk` form were used

### Requirement: Consistent diagnostic presentation for `zirk check`

The `zirk check` subcommand SHALL present diagnostics in the same format as `zirk build`, including human-readable and structured output, and SHALL route them to the same output stream.

#### Scenario: Human-readable error from `zirk check`
- **WHEN** `zirk check` emits a diagnostic without a structured-output flag
- **THEN** the rendered output contains severity, code, location, snippet, cause, and help in the same form as `zirk build`

#### Scenario: Structured output from `zirk check`
- **WHEN** `zirk check` is invoked with a structured-output flag
- **THEN** the output is the same machine-readable format as `zirk build`

