## MODIFIED Requirements

### Requirement: Crate structure per pipeline stage

The compiler SHALL be organized as a Cargo workspace with one crate per stage of the pipeline defined in `ZIRK_COMPILER_SPEC.md` section 2, plus diagnostics, platform-neutral runtime semantics, native runtime, IR interpreter, browser binding, formatting, and CLI crates.

The crates are: `zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-runtime-core`, `zirk-runtime`, `zirk-interpreter`, `zirk-browser`, `zirk-format`, and `zirk-cli`.

#### Scenario: Workspace builds
- **WHEN** `cargo build` is run at the repository root on a supported native host
- **THEN** every native-compatible workspace crate builds without errors
- **AND** `cargo clippy` reports no warnings

#### Scenario: Browser crates build
- **WHEN** the documented browser package build runs for `wasm32-unknown-unknown`
- **THEN** `zirk-runtime-core`, `zirk-interpreter`, `zirk-format`, and `zirk-browser` build without LLVM or native runtime machinery

#### Scenario: Declared responsibility
- **WHEN** the `lib.rs` of any crate in the workspace is inspected
- **THEN** it contains module documentation that declares its responsibility and its boundary

### Requirement: Single direction of dependencies

Dependencies between frontend pipeline crates SHALL flow in a single direction. `zirk-diagnostics` is the only cross-cutting compiler dependency. `zirk-codegen-llvm` and `zirk-interpreter` SHALL be sibling consumers of `zirk-ir`; neither execution engine SHALL depend on the other. `zirk-runtime` and `zirk-interpreter` MAY depend on `zirk-runtime-core`, while `zirk-runtime-core` SHALL NOT depend on either consumer. `zirk-format` SHALL depend only on the frontend crates it consumes (`zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-diagnostics`) and SHALL be consumable by `zirk-cli`, `zirk-lsp`, and `zirk-browser` without pulling in the backend.

#### Scenario: Backward dependency
- **WHEN** a crate from an early frontend stage declares a dependency on a later stage
- **THEN** it is considered a violation of this specification

#### Scenario: Dependency on diagnostics
- **WHEN** any compiler pipeline or execution-engine crate declares a dependency on `zirk-diagnostics`
- **THEN** it is valid regardless of its pipeline position

#### Scenario: Independent execution engines
- **WHEN** workspace dependencies are inspected
- **THEN** `zirk-interpreter` does not depend on `zirk-codegen-llvm`, `inkwell`, or `llvm-sys`
- **AND** `zirk-codegen-llvm` does not depend on `zirk-interpreter` or `zirk-browser`

#### Scenario: Shared runtime semantics
- **WHEN** native runtime and interpreter dependencies are inspected
- **THEN** both may reuse `zirk-runtime-core`
- **AND** platform-neutral runtime semantics do not depend on native ABI or browser bindings

#### Scenario: Formatter stays frontend-only
- **WHEN** `zirk-format` dependencies are inspected
- **THEN** it does not depend on `zirk-ir`, `zirk-codegen-llvm`, `zirk-runtime`, `zirk-interpreter`, or `zirk-browser`
- **AND** it performs no filesystem, network, or process access itself
