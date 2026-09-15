## ADDED Requirements

### Requirement: Browser compilation uses the authoritative in-memory pipeline

The browser compiler SHALL accept UTF-8 Zirk source from memory and reuse the authoritative lexer, parser, semantic checker, IR lowerer, and IR verifier. It SHALL NOT require filesystem access, a native process, LLVM, a linker, or the native runtime.

#### Scenario: Check valid source
- **WHEN** the browser `check` operation receives valid source within configured limits
- **THEN** it runs the authoritative frontend and returns a successful checked result with no runtime output

#### Scenario: Run valid source
- **WHEN** the browser `run` operation receives browser-compatible source with no compiler errors
- **THEN** it lowers and verifies the module and invokes the IR interpreter

#### Scenario: Receive compiler errors
- **WHEN** lexing, parsing, name resolution, or type checking produces an error
- **THEN** diagnostics identify their compiler stage and source spans
- **AND** lowering and execution do not begin

### Requirement: Browser operations return a versioned structured contract

Every browser request and response SHALL conform to a generated TypeScript-visible contract containing an API version, operation, compiler version, compiler revision, limits, completion state, diagnostics, ordered output events, and optional runtime failure. Unknown or incompatible API versions SHALL be rejected explicitly.

#### Scenario: Complete a successful run
- **WHEN** a compatible program terminates normally
- **THEN** the response identifies `run`, reports successful completion, includes ordered output, and contains package provenance

#### Scenario: Request an unsupported API version
- **WHEN** a caller submits a request version the package does not support
- **THEN** the operation returns an explicit contract-version error without compiling source

#### Scenario: Encounter an unexpected internal failure
- **WHEN** the browser boundary encounters an unexpected compiler or interpreter fault
- **THEN** it returns a sanitized `internal-error` result
- **AND** it does not expose native paths, host environment data, or a Rust panic payload as a normal diagnostic

### Requirement: Requests and results are bounded

The browser package SHALL define defaults and hard maxima for source bytes, diagnostics, instructions, call depth, interpreter heap, output bytes, output events, and reported stack frames. Caller-supplied limits SHALL only tighten package maxima.

#### Scenario: Source exceeds the hard maximum
- **WHEN** a request contains more UTF-8 source bytes than the package maximum
- **THEN** the package returns a `limit-exceeded` result naming `source`
- **AND** it does not begin lexing

#### Scenario: Caller requests a larger limit
- **WHEN** a request specifies a limit above the package maximum
- **THEN** the effective limit is capped or the request is rejected according to the versioned contract
- **AND** execution cannot exceed the package maximum

#### Scenario: Diagnostics exceed their limit
- **WHEN** compilation would emit more diagnostics than permitted
- **THEN** the response contains a deterministic bounded prefix and an explicit truncation indicator

### Requirement: Browser execution grants no ambient authority

The browser package SHALL neither request nor expose filesystem, network, environment, secrets, process, shell, native library, clock, random, or DOM capabilities. User source SHALL be processed only in caller-provided memory and SHALL not be persisted or transmitted by the package.

#### Scenario: Compile source locally
- **WHEN** a caller checks or runs source
- **THEN** the package performs no HTTP request, storage write, or process invocation

#### Scenario: Source requests an unavailable effect
- **WHEN** compilation produces a module requiring an unavailable host capability
- **THEN** the result contains a capability diagnostic
- **AND** the package does not simulate a successful effect

### Requirement: WebAssembly package is worker-compatible

The browser compiler SHALL build for `wasm32-unknown-unknown` and provide ES-module initialization and operation exports that can run inside a dedicated module Web Worker without DOM access. The package SHALL provide generated TypeScript declarations for its public contract.

#### Scenario: Initialize inside a worker
- **WHEN** a module worker loads the package's JavaScript entrypoint and WASM asset
- **THEN** initialization completes without `window`, `document`, or main-thread-only APIs

#### Scenario: Invoke before initialization
- **WHEN** a caller invokes an operation before successful initialization
- **THEN** the package returns or throws only the documented initialization error contract
- **AND** no partial compilation runs

### Requirement: Browser artifacts are reproducible and traceable

The repository SHALL provide one documented build command that emits the WASM binary, ES-module glue, TypeScript declarations, license material, and a manifest containing API version, compiler version, source revision, tool versions, supported operations, limits, exclusions, file sizes, and cryptographic checksums.

#### Scenario: Build from a clean pinned revision
- **WHEN** the documented browser-package command runs with the pinned toolchain
- **THEN** all required artifacts and the manifest are produced in the documented output directory

#### Scenario: Verify package integrity
- **WHEN** artifact checksums are recomputed after the build
- **THEN** they match the manifest values

#### Scenario: Artifact changes without provenance update
- **WHEN** a package file changes while its manifest checksum or source revision remains stale
- **THEN** package verification fails

### Requirement: Browser package excludes LLVM and native runtime machinery

The browser dependency graph and emitted package SHALL NOT include `inkwell`, `llvm-sys`, native linker drivers, native runtime C ABI entrypoints, native stdout, or native collector and pointer machinery.

#### Scenario: Audit the WASM dependency graph
- **WHEN** CI inspects dependencies for the browser target
- **THEN** LLVM and native-only runtime components are absent

#### Scenario: Build the native compiler
- **WHEN** the normal native workspace build runs
- **THEN** the LLVM backend remains available and its behavior is unchanged

### Requirement: Check and run share one frontend result model

The browser `check` and `run` operations SHALL use the same frontend driver and diagnostic conversion for identical source so their compiler-stage results cannot drift.

#### Scenario: Check and run invalid source
- **WHEN** identical invalid source is sent to `check` and `run`
- **THEN** both operations return equivalent ordered compiler diagnostics
- **AND** `run` returns no runtime output

### Requirement: Browser compatibility is explicitly discoverable

The package manifest and runtime contract SHALL identify supported operations and browser exclusions, and compatibility diagnostics SHALL distinguish unsupported browser tooling capabilities from invalid Zirk syntax or semantics.

#### Scenario: Inspect package capabilities
- **WHEN** a consumer reads the package manifest
- **THEN** it can determine that `check` and `run` are supported and that native-only effects are excluded

#### Scenario: Valid native program is incompatible with browser execution
- **WHEN** source is valid for the native compiler but requires an excluded browser capability
- **THEN** the result reports browser incompatibility rather than a language error
