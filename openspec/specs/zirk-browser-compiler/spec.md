# zirk-browser-compiler Specification

## Purpose

Define the versioned browser compiler package: an in-memory `check`/`run`
facade over the authoritative pipeline, built for `wasm32-unknown-unknown`
and distributed as a reproducible, auditable artifact for worker-hosted
consumers such as the documentation site playground.

The package is developer tooling, not a language target: it grants no ambient
authority, bounds every request and result, and proves that LLVM and the
native runtime stay out of the browser dependency graph.

## Requirements

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

The browser package SHALL neither request nor expose ambient filesystem, network, environment, secrets, process, shell, native library, clock, random, or DOM capabilities. User source SHALL be processed only in caller-provided memory and SHALL not be persisted or transmitted by the package. An embedding host MAY explicitly provide a per-execution browser-network capability with an origin allowlist; the compiler package SHALL pass it only to that execution and SHALL NOT persist it, widen it, or create a network request by itself.

#### Scenario: Compile source locally
- **WHEN** a caller checks or runs source without providing a network capability
- **THEN** the package performs no HTTP request, storage write, or process invocation

#### Scenario: Run with explicitly injected network authority
- **WHEN** an embedding host provides a bounded browser-network capability for one execution
- **THEN** only that execution may issue HTTP requests to its allowed origins through the browser transport
- **AND** the package does not gain filesystem, raw socket, server, environment, process, or DOM access

#### Scenario: Source requests an unavailable effect
- **WHEN** compilation produces a module requiring an unavailable host capability
- **THEN** the result contains a capability diagnostic
- **AND** the package does not simulate a successful effect

#### Scenario: A later execution follows an authorized execution
- **WHEN** another browser request runs after an execution that received network authority
- **THEN** it has no network authority unless the embedding host supplies a separate capability for that request

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

### Requirement: Browser format operation

The browser package SHALL expose a versioned `format` operation that
accepts UTF-8 Zirk source and returns either the canonically formatted
source or the ordered diagnostics for unformattable input. The operation
SHALL reuse the existing source-byte and diagnostic limits, SHALL appear
in the manifest's operation list and generated TypeScript declarations,
and SHALL run inside the worker without DOM, filesystem, or network
access. Adding the operation SHALL remain backward-compatible for
consumers of `check` and `run`.

#### Scenario: Format valid source in the worker

- **WHEN** a caller invokes `format` with well-formed source within limits
- **THEN** the response returns the canonically formatted source text

#### Scenario: Format invalid source

- **WHEN** `format` receives source with syntax errors
- **THEN** the response returns the ordered compiler diagnostics and no
  formatted text

#### Scenario: Manifest advertises format

- **WHEN** a consumer reads the package manifest
- **THEN** `format` appears alongside `check` and `run` in the operations
  list

#### Scenario: Older consumer unaffected

- **WHEN** a consumer built against the pre-`format` contract invokes
  `check` or `run`
- **THEN** both operations behave exactly as before

### Requirement: Browser run exposes an optional execution trace

The browser `run` operation SHALL accept an additive optional request field that enables the execution trace and selects event kinds, and SHALL return the trace as an additive response field only when requested. The trace limits SHALL be part of the reported limits, SHALL default to conservative values, and SHALL only be tightenable by the caller. The `apiVersion` SHALL remain `1`, and a response to a request that does not enable tracing SHALL be unchanged from the previous contract.

#### Scenario: Tracing not requested
- **WHEN** a caller runs source without enabling the trace
- **THEN** the response contains no trace field and is otherwise identical to the previous contract

#### Scenario: Tracing requested
- **WHEN** a caller runs a program with the trace enabled
- **THEN** the response contains ordered trace events with sequence numbers and a truncation flag

#### Scenario: Caller attempts to raise trace limits
- **WHEN** a request asks for trace limits above the package maxima
- **THEN** the effective limits are clamped to the package maxima

#### Scenario: Unknown trace option
- **WHEN** a request names an unknown event kind or trace field
- **THEN** the package returns a deterministic request failure and executes nothing

### Requirement: Execution trace grants no authority

The execution trace SHALL be an output-direction observation only. It SHALL NOT add input, filesystem, network, clock, random, DOM, or any other capability, SHALL NOT be reachable from interpreted code, and SHALL be discoverable in the package contract and manifest as an observation feature rather than a host effect.

#### Scenario: Program tries to read the trace
- **WHEN** interpreted source attempts to obtain the trace through any language construct or extern
- **THEN** no such construct exists or the extern fails preflight with a capability diagnostic

#### Scenario: Inspect package capabilities
- **WHEN** a consumer reads the package contract
- **THEN** it can determine that tracing is supported, which event kinds exist, and the trace limits

### Requirement: Browser stream capability boundary is explicit

The browser compiler SHALL represent `std.io` output as its existing bounded
output events and SHALL identify standard input reads as unavailable browser
effects before execution. It SHALL not acquire DOM, prompt, storage, network,
or other ambient input authority to emulate stdin.

#### Scenario: Captured output remains bounded
- **WHEN** browser execution writes through stdout or stderr within output limits
- **THEN** ordered events are returned through the versioned response contract

#### Scenario: Stdin is unavailable
- **WHEN** checked source has a reachable stdin read
- **THEN** browser execution reports a capability failure without invoking an ambient API
