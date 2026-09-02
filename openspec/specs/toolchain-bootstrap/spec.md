# toolchain-bootstrap

## Purpose

Define the reproducible toolchain used to build the Zirk compiler: which LLVM version is required, how it is located, and what evidence demonstrates that the full chain produces an executable native binary.

Its reason for being is that the backend is the highest-risk part of the project: a toolchain that cannot be reproduced on another machine invalidates everything built on top of it.
## Requirements
### Requirement: LLVM major version pin

The project SHALL build exclusively against LLVM 20.1.x via the `llvm20-1` feature of `inkwell` 0.10. A different major version of LLVM SHALL NOT be considered supported.

#### Scenario: Correct LLVM available
- **WHEN** `LLVM_SYS_201_PREFIX` points to an LLVM 20.1.x installation with static libraries
- **THEN** `cargo build` for the workspace completes without link errors

#### Scenario: Incorrect LLVM major version
- **WHEN** the environment provides a major LLVM version other than 20
- **THEN** the build SHALL fail with a diagnostic indicating the version found, the expected version, and a reference to `docs/TOOLCHAIN.md`
- **AND** the diagnostic SHALL NOT be a raw linker link error

#### Scenario: LLVM absent
- **WHEN** neither `LLVM_SYS_201_PREFIX` nor a compatible `llvm-config` on the `PATH` exists
- **THEN** the build SHALL fail indicating which environment variable to set

### Requirement: Full chain to a native binary

The toolchain SHALL demonstrate that it produces an executable native binary starting from LLVM IR built in-process, without depending on any Zirk syntax.

#### Scenario: Generation, linking, and execution
- **WHEN** the `zirk-codegen-llvm` sanity test is run
- **THEN** an LLVM module is built that verifies correctly
- **AND** an object file is emitted for the host target
- **AND** the object is linked into a native executable
- **AND** the executable runs and exits with code 0

### Requirement: Toolchain configuration not versioned per machine

The location of LLVM SHALL be resolved via an environment variable. No absolute path specific to a machine or platform SHALL be versioned in the repository.

#### Scenario: Absolute path in versioned configuration
- **WHEN** `.cargo/config.toml` or another versioned file defines `LLVM_SYS_201_PREFIX` with an absolute path
- **THEN** it is considered a violation of this specification

### Requirement: Installation documented per platform

The repository SHALL document how to obtain LLVM 20.1 with static libraries for macOS, Linux, and Windows.

#### Scenario: LLVM source on Windows
- **WHEN** a developer consults the installation documentation for Windows
- **THEN** the documentation SHALL explicitly state that **no** official LLVM distribution is compatible with `llvm-sys`
- **AND** it SHALL explain the two reasons: the `.exe` installer does not include static libraries, and the development tarball is built against a C runtime different from the one Rust uses
- **AND** it SHALL point to a source verified as compatible, at the pinned version

### Requirement: C runtime compatibility on Windows

The LLVM distribution used on Windows SHALL be built against the same C runtime used by Rust's `x86_64-pc-windows-msvc` target.

Mixing runtimes places two heaps within a single process: memory allocated inside LLVM and freed on the Rust side crosses the boundary and aborts the process.

#### Scenario: Incompatible runtime
- **WHEN** LLVM is built against a C runtime different from the one Rust uses
- **THEN** the compiler SHALL abort with an access violation on the first call to LLVM that returns a string
- **AND** the failure SHALL NOT manifest during linking, but at runtime

#### Scenario: Verification in continuous integration
- **WHEN** the continuous integration pipeline runs on Windows
- **THEN** the full test suite SHALL pass, including the sanity check that emits, links, and runs a native binary

### Requirement: Local verification reproduces CI's step order

`scripts/check-local.sh` SHALL run the same steps as
`.github/workflows/ci.yml`, in the same order: formatting, `clippy`, `cargo
build --workspace`, and then `cargo test --workspace`. The script SHALL NOT
skip the build step before tests.

`zirk-cli`'s end-to-end tests locate the `zirk` executable and the
runtime's static library under `cargo build`'s final output paths
(`target/<profile>/`), not under `cargo test`'s hashed paths in
`target/<profile>/deps/`. Without a prior `cargo build --workspace`, those
tests can either fail to find the artifact, or — in a tree with a stale
prior build — pass against a binary that no longer matches the current
source.

#### Scenario: Running against a clean working tree
- **WHEN** `./scripts/check-local.sh` is run in a freshly cloned copy with no prior `target/`
- **THEN** the script builds the full workspace before running the tests
- **AND** the script's result matches what CI would produce for the same commit

#### Scenario: An end-to-end test depends on the final build binary
- **WHEN** `crates/zirk-cli/tests/end_to_end.rs` looks up the `zirk` executable or `libzirk_runtime.a`/`zirk_runtime.lib`
- **THEN** it finds the artifact produced by the script's build step, not a stale or missing one

#### Scenario: Local script and CI diverge
- **WHEN** `.github/workflows/ci.yml` is modified to add, remove, or reorder a verification step
- **THEN** `scripts/check-local.sh` is updated in the same change to keep reproducing the same order

### Requirement: `zirk check` works without LLVM

The `zirk check` command SHALL execute and return useful results without `LLVM_SYS_201_PREFIX` set and without an LLVM installation on the host.

The LLVM major-version pin and the `llvm20-1` feature of `inkwell` SHALL apply only to commands that emit native code; `zirk check` is not one of those commands.

#### Scenario: Running `zirk check` without `LLVM_SYS_201_PREFIX`
- **WHEN** `zirk check` is executed with `LLVM_SYS_201_PREFIX` unset
- **THEN** the command runs the frontend and reports diagnostics
- **AND** it does not fail because LLVM is missing

#### Scenario: Building `zirk check` on a clean machine
- **WHEN** `cargo build -p zirk-cli --bin zirk-check --no-default-features` is run on a machine with no LLVM
- **THEN** the build succeeds
- **AND** the resulting binary can validate `.zrk` files

### Requirement: Toolchain documentation distinguishes validation from compilation

`docs/TOOLCHAIN.md` SHALL document that `zirk check` does not require LLVM, while `zirk build` and `zirk run` do.

#### Scenario: A new contributor reads the toolchain guide
- **WHEN** a contributor wants to validate a `.zrk` file without installing LLVM
- **THEN** the toolchain guide explicitly states that `zirk check` is available and does not need `LLVM_SYS_201_PREFIX`

