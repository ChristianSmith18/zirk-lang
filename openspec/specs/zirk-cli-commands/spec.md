# zirk-cli-commands

## Purpose

Defines the commands that compile and run a program, and how diagnostics are presented.
## Requirements
### Requirement: Format subcommand in the CLI

The CLI SHALL expose `zirk format <target>...` accepting one or more files
and directories; directories are scanned recursively for `.zrk` files.
`--check` reports required changes without writing and exits non-zero when
any target needs formatting; `--diff` prints the canonical changes without
writing.

#### Scenario: Formatting a file

- **WHEN** `zirk format main.zrk` runs on a well-formed file
- **THEN** it rewrites the file in canonical form

#### Scenario: Multiple targets

- **WHEN** `zirk format a.zrk b.zrk src/` runs
- **THEN** every listed file and every `.zrk` file under `src/` is
  formatted, and each target reports whether it changed

#### Scenario: Check mode over several files

- **WHEN** `zirk format --check` runs over multiple targets
- **THEN** it exits non-zero if any target would change and names them

#### Scenario: Diff mode writes nothing

- **WHEN** `zirk format --diff` runs
- **THEN** it prints the canonical diff for each target and writes nothing

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

The CLI SHALL expose a subcommand that compiles and runs a program in a single step. `zirk run <path>` SHALL execute the given source file, with or without the `.zrk` extension. When the source path is omitted, `zirk run` SHALL discover the nearest applicable application `.zkinit` and execute its configured or default entry. An explicit path inside a project SHALL override only the selected entry for that invocation while retaining project dependencies, lockfile, globals, and capability requirements. A file without a project manifest MAY run as a standalone entry under standalone import rules.

#### Scenario: Execution after compiling with extension
- **WHEN** a valid program file is run using the `.zrk` extension
- **THEN** the program runs and its output appears on standard output

#### Scenario: Execution after compiling without extension
- **WHEN** a valid program file is run without the `.zrk` extension
- **THEN** the CLI resolves the source file and runs it

#### Scenario: Run uses manifest entry
- **WHEN** `zirk run` is invoked without a source argument inside an application project
- **THEN** the CLI executes the manifest-selected application entry

#### Scenario: Explicit file retains project context
- **WHEN** `zirk run src/dev.zrk` is invoked inside an application project
- **THEN** `src/dev.zrk` executes with the project's manifest, lockfile, globals, dependencies, and requirements

#### Scenario: Package project has no default application entry
- **WHEN** `zirk run` is invoked without a source argument inside a package project
- **THEN** the CLI reports that packages require an explicit executable source path

#### Scenario: Exit code propagation
- **WHEN** the compiled program exits with a process status
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

The CLI SHALL support standalone single-file execution as well as manifest-backed projects. An explicit source path without an applicable `.zkinit` SHALL proceed under standalone import rules; an explicit source path within a project SHALL use that project's manifest and lockfile context. `zirk run` without a path SHALL require an application manifest. Package projects and missing project manifests SHALL produce targeted diagnostics when no explicit source path is supplied.

#### Scenario: Standalone explicit file
- **WHEN** a source file is run without an applicable `.zkinit`
- **THEN** it runs as a standalone entry without project dependency lookup

#### Scenario: Absence of a manifest
- **WHEN** a source file is explicitly compiled or run without a `.zkinit`
- **THEN** compilation proceeds normally under standalone import rules

#### Scenario: Project explicit file
- **WHEN** a source file inside a project is run explicitly
- **THEN** the nearest applicable project configuration is loaded before compilation

#### Scenario: Run without manifest
- **WHEN** `zirk run` is invoked without a source path and no project manifest exists
- **THEN** the CLI reports that an application project entry cannot be discovered

#### Scenario: Multiple files
- **WHEN** several source files are supplied where one entry path is expected
- **THEN** the CLI emits a diagnostic explaining that one selected entry path is required

### Requirement: Authority-bearing commands validate signed approval incrementally
Before compiling or running a manifest-backed project, relevant CLI commands SHALL validate the project identity, manifest syntax, lockfile, and dependency/requester graph and SHALL reuse cached approval fingerprints when inputs are unchanged. A missing consent record SHALL NOT prevent application startup or compilation solely because a requirement is declared; the runtime SHALL check concrete resources immediately before protected operations. A changed or unverifiable approval record SHALL grant nothing for the affected scope and SHALL trigger a fresh runtime decision or non-interactive denial. The CLI SHALL NOT execute user code to resolve an invalid manifest or dependency graph.

#### Scenario: Dependency requester updated
- **WHEN** an approved dependency changes version, integrity, or requirement provenance
- **THEN** the approval fingerprint no longer authorizes the changed concrete request and the runtime checks it again at operation time

#### Scenario: No permission has been approved yet
- **WHEN** an application starts with declared requirements and no matching saved consent
- **THEN** unprotected code can run and no startup-wide permission error is raised

### Requirement: Permission management commands are auditable
The CLI SHALL provide inspection, diff, approval, revocation, and history operations for saved capability consent. Interactive approval SHALL show the exact resource and requester/dependency path. Non-interactive execution SHALL NOT prompt or infer consent; an unapproved protected operation SHALL be denied at its operation boundary and produce a machine-readable authorization diagnostic. Consent inspection and revocation SHALL operate on the trusted approval record and SHALL NOT introduce a second manifest permission block.

#### Scenario: Non-interactive run reaches an unapproved operation
- **WHEN** CI runs an application that reaches a protected resource without a matching saved approval
- **THEN** that operation is denied without prompting and the process can continue if the program handles its result

#### Scenario: Noninteractive build lacks approval
- **WHEN** CI builds an application that declares runtime capabilities but executes no protected operation during compilation
- **THEN** the build is not blocked by missing runtime consent and a later unapproved runtime operation is denied at its operation boundary

#### Scenario: User inspects an approval
- **WHEN** the user requests capability history for a project
- **THEN** the CLI identifies project, requester, concrete scope, dependency path, approver, and timestamp without exposing secrets

#### Scenario: User revokes an approval
- **WHEN** an approval is revoked
- **THEN** the next protected operation using that scope is denied or prompts according to execution mode

### Requirement: Initialize a canonical project manifest

The CLI SHALL provide `zirk init` to create a canonical `.zkinit` in the current directory. In an interactive terminal it SHALL ask, one field at a time in the style of `npm init`, for the project type, name, version, type-specific `entry` or `root`, optional authors/description/license, source roots, and application build targets. Each prompt SHALL show its default; pressing Enter SHALL accept that value, while supplied input SHALL replace it. Optional metadata or build targets MAY be left unset. Dependencies, development dependencies, globals, and capability requirements SHALL start empty; initialization SHALL NOT infer consent or create approval records. The command SHALL validate and format the typed manifest before atomically creating the file, SHALL NOT create source files, and SHALL refuse to overwrite an existing `.zkinit`.

`zirk init -y` SHALL skip all prompts and accept the documented default for every field: application type, a normalized current-directory name (or `zirk-project` when no valid name can be derived), version `0.1.0`, entry `src/main.zrk`, and source root `src`. Defaults for optional metadata and application build targets are unset, so those fields SHALL be omitted and target selection SHALL use host detection. Dependencies, development dependencies, globals, and requirements SHALL remain empty. For interactive package selection, the default root SHALL be `src/package.zrk` and the application-only build target block SHALL be omitted. A failed, canceled, or invalid initialization SHALL leave no partial manifest.

#### Scenario: Interactive project initialization
- **WHEN** a user runs `zirk init` in a directory without `.zkinit` and answers the project prompts
- **THEN** the CLI writes a formatted manifest using the selected type and answers, with unanswered optional fields omitted
- **AND** the result passes the canonical manifest parser and project-type validation

#### Scenario: Initialize with defaults
- **WHEN** a user runs `zirk init -y` in a directory without `.zkinit`
- **THEN** the CLI creates an application manifest with the documented defaults without prompting
- **AND** empty optional sections do not create dependencies, globals, requirements, or consent

#### Scenario: Package initialization chooses a package root
- **WHEN** an interactive user selects `package` and accepts the default root
- **THEN** the generated manifest sets `project.root` to `src/package.zrk` and omits application-only `entry` and `build_targets`

#### Scenario: Existing project manifest is protected
- **WHEN** `zirk init` or `zirk init -y` runs in a directory that already contains `.zkinit`
- **THEN** the CLI reports that the manifest already exists and leaves it unchanged

#### Scenario: Initialization is canceled or invalid
- **WHEN** a user cancels the prompts or enters data that fails canonical manifest validation
- **THEN** the CLI reports the issue and leaves no partial `.zkinit`

#### Scenario: Non-interactive initialization requires defaults mode
- **WHEN** `zirk init` runs without an interactive terminal and without `-y`
- **THEN** the CLI reports that interactive input is unavailable and creates no files

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
