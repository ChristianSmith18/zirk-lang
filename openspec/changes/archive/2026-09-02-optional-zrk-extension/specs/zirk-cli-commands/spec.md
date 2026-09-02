## MODIFIED Requirements

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
