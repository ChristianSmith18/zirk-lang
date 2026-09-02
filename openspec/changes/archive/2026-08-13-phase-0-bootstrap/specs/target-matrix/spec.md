## ADDED Requirements

### Requirement: Object emission for the spec's targets

The backend SHALL emit valid object files for the targets in `ZIRK_COMPILER_SPEC.md` section 6, from any supported host. This capability corresponds to *portability B* from [ADR-004](../../../../docs/decisions/ADR-004-portabilidad.md): what the compiler produces.

Targets covered: `x86-windows`, `x86_64-windows`, `aarch64-windows`, `x86-linux`, `x86_64-linux`, `armv7-linux`, `aarch64-linux`, `x86_64-macos`, `aarch64-macos`.

#### Scenario: Cross-target emission from a single host
- **WHEN** the target matrix test is run on any supported platform
- **THEN** an object file is emitted for each of the nine targets
- **AND** each object has the correct container format: Mach-O for macOS, ELF for Linux, COFF for Windows
- **AND** each object declares the architecture matching the target

#### Scenario: Target not supported by the backend
- **WHEN** emission is requested for a triple that LLVM does not recognize
- **THEN** the compiler SHALL fail with a diagnostic naming the requested target
- **AND** SHALL NOT produce an invalid object

### Requirement: Building the compiler on all three platforms

The compiler SHALL be buildable from source on Windows, Linux, and macOS. This capability corresponds to *portability A* from ADR-004: where the compiler is built.

#### Scenario: Continuous verification in integration
- **WHEN** the continuous integration pipeline is run
- **THEN** the workspace builds and its tests pass on Linux (x86_64 and aarch64), macOS (x86_64 and aarch64), and Windows (x86_64)

#### Scenario: Failure on a platform
- **WHEN** the workspace fails to build or its tests fail on any platform in the matrix
- **THEN** portability is considered unverified
- **AND** the finding SHALL be treated as blocking, not as pending

### Requirement: Single, cross-platform linker

The project SHALL use `lld` as the reference linker, covering ELF, Mach-O, and COFF from a single version, instead of relying on each host's default linker.

#### Scenario: Driver availability
- **WHEN** the toolchain installation is verified
- **THEN** `lld` provides the drivers for the three required container formats

### Requirement: Cross-linking out of scope at this phase

Producing a complete executable for a target other than the host SHALL be out of scope at this phase, since it requires destination sysroots.

#### Scenario: Scope of target verification
- **WHEN** the target matrix is verified
- **THEN** the verification covers object emission
- **AND** does NOT cover linking executables for targets other than the host
