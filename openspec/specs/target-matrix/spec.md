# target-matrix

## Purpose

Define which targets the compiler must be able to emit and on which platforms it must be able to build, along with their verification criteria.

It distinguishes two kinds of portability that do not cost the same: producing binaries for other platforms, and building the compiler on them. See `docs/decisions/ADR-004-portabilidad.md`.

## Requirements

### Requirement: Object emission for the spec's targets

The backend SHALL emit valid object files for the targets in `ZIRK_COMPILER_SPEC.md` section 6, from any supported host. This capability corresponds to *portability B* from [ADR-004](../../../../docs/decisions/ADR-004-portabilidad.md): what the compiler produces.

Covered targets: `x86-windows`, `x86_64-windows`, `aarch64-windows`, `x86-linux`, `x86_64-linux`, `armv7-linux`, `aarch64-linux`, `x86_64-macos`, `aarch64-macos`.

#### Scenario: Cross-target emission from a single host
- **WHEN** the target matrix test is run on any supported platform
- **THEN** an object file is emitted for each of the nine targets
- **AND** each object has the correct container format: Mach-O for macOS, ELF for Linux, COFF for Windows
- **AND** each object declares the architecture corresponding to the target

#### Scenario: Target not supported by the backend
- **WHEN** emission is requested for a triple that LLVM does not recognize
- **THEN** the compiler SHALL fail with a diagnostic naming the requested target
- **AND** it SHALL NOT produce an invalid object

### Requirement: Building the compiler on the three platforms

The compiler SHALL be buildable from source on Windows, Linux, and macOS. This capability corresponds to *portability A* from ADR-004: where the compiler is built.

#### Scenario: Continuous verification in integration
- **WHEN** the continuous integration pipeline runs
- **THEN** the workspace builds and its tests pass on Linux (x86_64 and aarch64), macOS (x86_64 and aarch64), and Windows (x86_64)

#### Scenario: Failure on one platform
- **WHEN** the workspace fails to build or its tests fail on any platform in the matrix
- **THEN** portability is considered unverified
- **AND** the finding SHALL be treated as blocking, not as pending

### Requirement: Single, cross-platform linker

The project SHALL use `lld` as the reference linker, covering ELF, Mach-O, and COFF from a single version, instead of relying on each host's default linker.

#### Scenario: Driver availability
- **WHEN** the toolchain installation is verified
- **THEN** `lld` provides the drivers for the three required container formats

### Requirement: Cross-linking out of scope in this phase

Producing a complete executable for a target other than the host SHALL be out of scope in this phase, since it requires target sysroots.

#### Scenario: Scope of target verification
- **WHEN** the target matrix is verified
- **THEN** verification covers object emission
- **AND** it does NOT cover linking executables for targets other than the host

### Requirement: The produced executable runs on the host

Verification SHALL check that the executable generated from Zirk code runs correctly on the host platform, not merely that the object file is emitted.

Through Phase 0, only object emission and a binary built directly from Rust were verified. With a real language ahead, the evidence that matters is that a `.zrk` file ends up as a process that runs.

#### Scenario: Reference program
- **WHEN** a `.zrk` program that prints a string is compiled and run
- **THEN** the process exits with code 0
- **AND** the standard output contains exactly the expected string

#### Scenario: Verification on the supported platforms
- **WHEN** the continuous integration pipeline runs
- **THEN** compiling and running the reference program is verified on each platform in the matrix
