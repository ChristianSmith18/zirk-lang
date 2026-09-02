## ADDED Requirements

### Requirement: LLVM major version pin

The project SHALL build exclusively against LLVM 20.1.x via the `llvm20-1` feature of `inkwell` 0.10. Any different major version of LLVM SHALL NOT be considered supported.

#### Scenario: Correct LLVM available
- **WHEN** `LLVM_SYS_201_PREFIX` points to an LLVM 20.1.x installation with static libraries
- **THEN** `cargo build` of the workspace completes with no link errors

#### Scenario: Incorrect LLVM major version
- **WHEN** the environment provides a major LLVM version different from 20
- **THEN** the build SHALL fail with a diagnostic indicating the version found, the version expected, and a reference to `docs/TOOLCHAIN.md`
- **AND** the diagnostic SHALL NOT be a raw linker error

#### Scenario: LLVM missing
- **WHEN** neither `LLVM_SYS_201_PREFIX` nor a compatible `llvm-config` exists on the `PATH`
- **THEN** the build SHALL fail indicating which environment variable to define

### Requirement: Full chain to a native binary

The toolchain SHALL demonstrate that it produces an executable native binary starting from LLVM IR built in-process, without depending on any Zirk syntax.

#### Scenario: Generation, linking, and execution
- **WHEN** the `zirk-codegen-llvm` sanity test is run
- **THEN** an LLVM module is built that verifies correctly
- **AND** an object file is emitted for the host target
- **AND** the object is linked into a native executable
- **AND** the executable runs and exits with exit code 0

### Requirement: Toolchain configuration not versioned per machine

The location of LLVM SHALL be resolved via an environment variable. No absolute path specific to a machine or platform SHALL be versioned in the repository.

#### Scenario: Absolute path in versioned configuration
- **WHEN** `.cargo/config.toml` or another versioned file defines `LLVM_SYS_201_PREFIX` with an absolute path
- **THEN** it is considered a violation of this specification

### Requirement: Installation documented per platform

The repository SHALL document how to obtain LLVM 20.1 with static libraries for macOS, Linux, and Windows.

#### Scenario: Warning about the Windows installer
- **WHEN** a developer consults the installation documentation for Windows
- **THEN** the documentation SHALL explicitly state that the official LLVM `.exe` installer does not include the required static libraries
- **AND** SHALL direct to the official development tarball `clang+llvm-20.1.8-*-pc-windows-msvc.tar.xz`
