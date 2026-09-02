## ADDED Requirements

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
