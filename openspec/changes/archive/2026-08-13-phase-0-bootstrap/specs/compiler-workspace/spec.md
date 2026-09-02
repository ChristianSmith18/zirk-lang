## ADDED Requirements

### Requirement: Crate structure per pipeline stage

The compiler SHALL be organized as a Cargo workspace with one crate per pipeline stage from `ZIRK_COMPILER_SPEC.md` section 2, plus a diagnostics crate and a runtime crate.

The crates are: `zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-cli`, and `zirk-runtime`.

#### Scenario: Workspace builds
- **WHEN** `cargo build` is run at the repository root
- **THEN** the nine crates build without errors
- **AND** `cargo clippy` reports no warnings

#### Scenario: Declared responsibility
- **WHEN** the `lib.rs` of any workspace crate is inspected
- **THEN** it contains module documentation that declares its responsibility and its boundary

### Requirement: Single direction of dependencies

Dependencies between crates SHALL flow in a single direction along the pipeline. `zirk-diagnostics` is the only permitted cross-cutting dependency.

#### Scenario: Backward dependency
- **WHEN** a crate from an early stage declares a dependency on a crate from a later stage
- **THEN** it is considered a violation of this specification

#### Scenario: Dependency on diagnostics
- **WHEN** any pipeline crate declares a dependency on `zirk-diagnostics`
- **THEN** it is valid, regardless of its position in the pipeline

### Requirement: Runtime independence from the compiler

`zirk-runtime` SHALL be compiled as a `staticlib` and SHALL NOT be a dependency of any compiler crate. It is linked into the binaries Zirk produces, not into the compiler itself.

#### Scenario: Produced artifact
- **WHEN** `zirk-runtime` is built
- **THEN** it produces a linkable static library (`.a` on Unix, `.lib` on Windows)

#### Scenario: C ABI boundary
- **WHEN** `zirk-runtime` exposes a symbol intended for generated code
- **THEN** the symbol SHALL be declared `extern "C"` with a stable name and no mangling

#### Scenario: Application lifecycle
- **WHEN** the public surface of `zirk-runtime` is inspected
- **THEN** it exposes `zirk_rt_init` and `zirk_rt_shutdown`, corresponding to the lifecycle from `ZIRK_RUNTIME_SPEC.md` section 2

### Requirement: Absence of Zirk syntax at this phase

The workspace at this phase SHALL NOT implement lexical, syntactic, or semantic analysis of Zirk code.

#### Scenario: Attempt to compile a source file
- **WHEN** functionality to process a `.zrk` file is sought
- **THEN** it does not exist in this phase's workspace
