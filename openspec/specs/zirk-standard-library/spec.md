# zirk-standard-library Specification

## Purpose

Defines the standard-library module inventory and the shared contracts for API
signatures, outcomes, permissions, cancellation, hostile-input limits,
portability, and documentation synchronization.

## Requirements

### Requirement: The standard library has one synchronized capability owner
The standard-library capability SHALL inventory every public `std` module and
shared API contract defined by `docs/ZIRK_STDLIB_SPEC.md`. Its handbook owner
pages SHALL use the same signatures, result variants, permissions,
cancellation behavior, safety limits, and platform guarantees.

#### Scenario: A module API changes
- **WHEN** a normative standard-library signature or behavior changes
- **THEN** its specialized spec, OpenSpec capability, handbook owner, indexes,
  and affected examples are updated in the same change

### Requirement: Result and stream outcomes remain distinct
Operations returning `Result<T,E>` SHALL use `Ok(T)` and `Error(E)`. A
stream-specific enum MAY use `Value(T)`, `End`, and `Error(E)` only when the
operation explicitly returns that enum, such as `ReadResult<T>` or
`ReceiveResult<T>`.

#### Scenario: HTTP request succeeds
- **WHEN** an HTTP operation returning `Result<HTTPResponse,HTTPError>` succeeds
- **THEN** documentation and implementations expose `Ok(response)`, not
  `Value(response)`

### Requirement: Privileged APIs preserve denial explicitly
Filesystem, network, process, environment, system, and secret APIs SHALL retain
permission, decoding, platform, and cancellation failures in their declared
typed outcome. Nullable/default convenience operations SHALL NOT hide denial
or host failure.

#### Scenario: Environment fallback lacks authority
- **WHEN** `Env.get_or(name, fallback)` has no matching grant
- **THEN** it returns `Error(EnvironmentPermissionError)` rather than the
  fallback
