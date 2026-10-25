## MODIFIED Requirements

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
- **THEN** the CLI exits with that same status

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

## ADDED Requirements

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
