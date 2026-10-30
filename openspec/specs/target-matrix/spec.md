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

### Requirement: Float128 is verified across the supported target matrix
The target verification matrix SHALL compile `Float128` arithmetic, conversion, comparison, and formatting for every object-emission target defined by the compiler specification. Native integration SHALL run precision and boundary fixtures on each platform where the compiler workspace is natively built, including Windows x86_64. Target coverage SHALL report object-emission and native-run evidence separately; cross-target object emission SHALL NOT be reported as runtime verification.

#### Scenario: Float128 object emission covers every target
- **WHEN** the target-matrix workflow emits objects for all compiler-supported target triples
- **THEN** each target emits valid code for the documented `Float128` operation fixture or reports a target-specific blocking failure

#### Scenario: Float128 runs on Windows x86_64
- **WHEN** the native Windows x86_64 integration workflow compiles and runs the precision and boundary fixtures
- **THEN** linker resolution, execution, and output satisfy the binary128 contract

#### Scenario: Native evidence is reported distinctly
- **WHEN** the target matrix report summarizes Float128 coverage
- **THEN** it distinguishes cross-target object emission from executable runs on native CI platforms

### Requirement: Advanced type features have stage-specific target evidence

The support matrix for generic enum dispatch, admitted union/intersection combinations, aggregate `Hash`/`Equal`, and enum `Clone` SHALL report separate results for parsing/type checking, lowering, interpreter execution, object emission, native linking, and native execution. Native execution SHALL be verified on every supported CI host: Linux x86_64 and aarch64, macOS x86_64 and aarch64, and Windows x86_64. Object emission SHALL cover all nine compiler targets already defined by this capability. Evidence for one host or one pipeline stage SHALL NOT imply another host or stage.

#### Scenario: Native matrix runs the advanced feature corpus

- **WHEN** the supported native CI matrix runs the advanced type corpus
- **THEN** each host records the compiler/runtime revision and passes its executable behavior cases or is reported as an explicit failure

#### Scenario: Object-emission matrix includes generic enums

- **WHEN** the nine-target object-emission matrix compiles the generic enum and algebraic payload corpus
- **THEN** each target emits a valid object or reports a target-specific unsupported capability before emitting invalid output

#### Scenario: Evidence labels distinguish runtime from emission

- **WHEN** a target has object-emission evidence but no native host execution
- **THEN** its published status says object emission only and does not claim runtime execution

### Requirement: Float128 target availability is explicit and verified

`Float128` SHALL either be supported through type checking, lowering, runtime operations, and executable behavior on Windows x86_64, or the source contract and feature status SHALL declare its precise Windows exclusion and reject unsupported use with a source diagnostic. Object emission alone SHALL NOT establish Windows runtime support.

#### Scenario: Float128 Windows behavior is supported

- **WHEN** the Windows x86_64 native CI target compiles and runs the Float128 boundary and arithmetic corpus
- **THEN** results match the language contract and the status catalog records native execution evidence

#### Scenario: Float128 Windows use is excluded

- **WHEN** Float128 is outside the declared Windows contract
- **THEN** a source-located diagnostic rejects each unsupported use before backend emission and the status catalog names the exact exclusion
