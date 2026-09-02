# compiler-workspace

## Purpose

Define the crate structure of the compiler, the responsibility boundary of each one, and the dependency rule between pipeline stages.

The structure directly mirrors the pipeline of `ZIRK_COMPILER_SPEC.md` section 2, so that each stage can be tested in isolation and the backend stays contained within a single crate.
## Requirements
### Requirement: Crate structure per pipeline stage

The compiler SHALL be organized as a Cargo workspace with one crate per stage of the pipeline defined in `ZIRK_COMPILER_SPEC.md` section 2, plus a diagnostics crate and a runtime crate.

The crates are: `zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-cli`, and `zirk-runtime`.

#### Scenario: Workspace builds
- **WHEN** `cargo build` is run at the repository root
- **THEN** the nine crates build without errors
- **AND** `cargo clippy` reports no warnings

#### Scenario: Declared responsibility
- **WHEN** the `lib.rs` of any crate in the workspace is inspected
- **THEN** it contains module documentation that declares its responsibility and its boundary

### Requirement: Single direction of dependencies

Dependencies between crates SHALL flow in a single direction along the pipeline. `zirk-diagnostics` is the only allowed cross-cutting dependency.

#### Scenario: Backward dependency
- **WHEN** a crate from an early stage declares a dependency on a crate from a later stage
- **THEN** it is considered a violation of this specification

#### Scenario: Dependency on diagnostics
- **WHEN** any crate in the pipeline declares a dependency on `zirk-diagnostics`
- **THEN** it is valid, regardless of its position in the pipeline

### Requirement: Runtime independence from the compiler

`zirk-runtime` SHALL be built as a `staticlib` and SHALL NOT be a dependency of any compiler crate. It is linked into the binaries that Zirk produces, not into the compiler itself.

#### Scenario: Produced artifact
- **WHEN** `zirk-runtime` is built
- **THEN** it produces a linkable static library (`.a` on Unix, `.lib` on Windows)

#### Scenario: C ABI boundary
- **WHEN** `zirk-runtime` exposes a symbol intended for generated code
- **THEN** the symbol SHALL be declared `extern "C"` with a stable, unmangled name

#### Scenario: Application lifecycle
- **WHEN** the public surface of `zirk-runtime` is inspected
- **THEN** it exposes `zirk_rt_init` and `zirk_rt_shutdown`, corresponding to the lifecycle defined in `ZIRK_RUNTIME_SPEC.md` section 2

### Requirement: Pipeline coverage per crate

Each stage of the pipeline in `ZIRK_COMPILER_SPEC.md` section 2 SHALL be implemented in its corresponding crate, without one stage taking on the responsibilities of another.

#### Scenario: The lexer does not know the grammar
- **WHEN** `zirk-lexer` is inspected
- **THEN** it produces tokens
- **AND** it does NOT decide whether a sequence of tokens is valid

#### Scenario: The parser does not check types
- **WHEN** an expression with incompatible types but correct syntax is parsed
- **THEN** the parser produces the tree without error
- **AND** the type error is emitted by the type checker

#### Scenario: The backend is the only crate that knows about LLVM
- **WHEN** the crates in the workspace are inspected
- **THEN** only `zirk-codegen-llvm` depends on `inkwell`

#### Scenario: Linking does not happen in the backend
- **WHEN** an executable is produced
- **THEN** the backend emits the object
- **AND** the linker invocation happens in the CLI

### Requirement: Frontend-only build graph

The workspace SHALL support a build configuration that compiles the `zirk check` command using only the frontend crates (`zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-diagnostics`) and the CLI crate, without pulling in `zirk-ir`, `zirk-codegen-llvm`, or `zirk-runtime`.

#### Scenario: Build `zirk-check` binary without the backend
- **WHEN** `cargo build -p zirk-cli --bin zirk-check --no-default-features` is executed
- **THEN** the build completes
- **AND** it does not compile or link `zirk-ir`, `zirk-codegen-llvm`, or `zirk-runtime`

### Requirement: Shared frontend driver for both binaries

`zirk-cli` SHALL provide a single internal driver that runs the frontend and returns a result. Both the `zirk` and `zirk-check` binaries SHALL invoke this driver for the same input, so that `zirk build` and `zirk check` cannot diverge on the frontend semantics.

#### Scenario: Same input reaches the same semantic result
- **WHEN** the same `.zrk` file is given to `zirk check` and `zirk build`
- **THEN** both commands observe the same set of diagnostics from the frontend stages
- **AND** any divergence is treated as a bug in the driver or in the split itself

### Requirement: Backend feature is optional for the check binary

The `backend` Cargo feature in `zirk-cli` SHALL gate the inclusion of `zirk-ir`, `zirk-codegen-llvm`, the runtime archive, and the `zirk build`/`zirk run` commands. The `zirk-check` binary SHALL NOT require the `backend` feature.

#### Scenario: Default build still supports the full compiler
- **WHEN** `cargo build -p zirk-cli` runs with default features
- **THEN** the `zirk` binary is built with the full backend and remains capable of producing executables

#### Scenario: Check binary builds without backend
- **WHEN** the `backend` feature is disabled
- **THEN** the `zirk-check` binary still builds and runs the frontend

