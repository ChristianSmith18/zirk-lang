## ADDED Requirements

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
