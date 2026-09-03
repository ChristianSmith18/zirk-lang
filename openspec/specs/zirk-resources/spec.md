# zirk-resources Specification

## Purpose
Defines deterministic resource acquisition, cleanup, failure composition,
responsibility transfer, and lifetime enforcement.
## Requirements
### Requirement: Match with closes successful acquisitions exactly once

The language SHALL define `Resource<E from Error>` with typed `close` and
closed-state behavior. `match with` SHALL own each successful acquisition and
close it exactly once on normal completion, return, exception, or control
transfer leaving the scope.

The final language contract includes single and grouped acquisition,
cancellation-aware cleanup, surfaced close failures, and explicit transfer.
Current compiler coverage is tracked in the handbook feature-status page and
phase closeouts rather than by narrowing this normative requirement.

#### Scenario: Body returns early

- **WHEN** a `match with` success branch returns before its final statement
- **THEN** the owned resource closes before the return value leaves the scope

#### Scenario: Exception propagates through the owning branch

- **WHEN** a `match with` success branch calls a function whose exception
  propagates past it uncaught
- **THEN** the owned resource closes before the exception continues
  propagating

### Requirement: Multiple resources acquire and unwind deterministically
Grouped resources SHALL acquire left-to-right and close right-to-left. Failure to acquire a later resource SHALL unwind every earlier successful acquisition before entering the error branch.

#### Scenario: Second acquisition fails
- **WHEN** the first resource opens and the second acquisition returns `Error`
- **THEN** the first resource closes before the acquisition error is delivered

### Requirement: Cleanup failures preserve every outcome
A close failure after body success SHALL become the block failure. A close failure during exception propagation SHALL be appended to the primary throwable's suppressed list. A close failure combined with `Result.Error` SHALL be represented by `ResourceFailure<BodyError,CloseError>` with distinct `Body`, `Close`, and `BodyAndClose` variants.

#### Scenario: Body and close both return errors
- **WHEN** a managed body returns `Error(body)` and closing returns `Error(close)`
- **THEN** the result is `ResourceFailure.BodyAndClose(body, close)` and neither failure is discarded

### Requirement: Transfer and duplication are explicit
Only a resource satisfying `TransferableResource` SHALL support `transfer()`. Transfer SHALL invalidate the source responsibility and require the receiver to close, re-manage, or transfer the resource. `Resource` SHALL NOT imply `Clone`; handle duplication or splitting SHALL use explicit fallible type-specific APIs.

#### Scenario: Original binding used after transfer
- **WHEN** code uses a resource binding after a statically visible `transfer()`
- **THEN** compilation fails with a transferred-resource diagnostic

### Requirement: Resource lifetime and dependency are enforced
Resources managed by a scope SHALL NOT escape through a return, collection, object, closure, or task unless explicitly transferred under a supported contract. Dependent resources SHALL not outlive parents. Provable abandonment SHALL be a compile-time error; dynamic closed/transferred misuse SHALL produce typed runtime failure rather than undefined behavior.

#### Scenario: Resource projected from a container
- **WHEN** code attempts ordinary projection-copy extraction of a non-cloneable resource
- **THEN** compilation fails and suggests a responsibility-moving operation such as `take`

### Requirement: Grouped match-with acquires left-to-right and unwinds right-to-left
The language SHALL support `match r1 with ..., r2 with ...` and SHALL acquire the resources from left to right. If a later acquisition fails, the language SHALL close every earlier successful acquisition in reverse order before entering the error branch.

#### Scenario: Second acquisition fails
- **WHEN** the first resource opens successfully and the second acquisition returns `Error`
- **THEN** the first resource is closed and the error branch receives the second error

#### Scenario: Third acquisition fails
- **WHEN** the first two resources open and the third acquisition returns `Error`
- **THEN** the second and first resources are closed in that order before the error branch runs

### Requirement: Close failures are surfaced and merged
The language SHALL combine a managed body failure and a cleanup close failure into a `ResourceFailure<BodyError, CloseError>` value with `Body`, `Close`, and `BodyAndClose` variants, preserving both failures.

#### Scenario: Body and close both fail
- **WHEN** a managed body returns `Error(body)` and closing returns `Error(close)`
- **THEN** the block result is `ResourceFailure.BodyAndClose(body, close)`

#### Scenario: Only close fails
- **WHEN** the body succeeds but closing returns `Error(close)`
- **THEN** the block result is `ResourceFailure.Close(close)`

### Requirement: TransferableResource supports transfer
The language SHALL allow `transfer(r)` only for a resource that implements `TransferableResource`; the operation SHALL invalidate the source binding and the receiver SHALL assume ownership.

#### Scenario: Transfer moves ownership
- **WHEN** `mut moved = transfer(r)` is used with a `TransferableResource`
- **THEN** `moved` owns the resource and any later use of `r` is rejected at compile time

#### Scenario: Non-transferable resource is transferred
- **WHEN** `transfer(r)` is used on a resource that does not implement `TransferableResource`
- **THEN** compilation fails

### Requirement: Dependent resources cannot outlive parents
The language SHALL reject any dependent resource use that may outlive its parent through return, field store, closure capture, or task spawn.

#### Scenario: Dependent resource escapes
- **WHEN** a dependent resource is returned from a function
- **THEN** compilation fails with a dependent-lifetime diagnostic

#### Scenario: Dependent resource outlives parent in a container
- **WHEN** a dependent resource is stored in a field of an object that may live longer than the parent
- **THEN** compilation fails

### Requirement: Cancellation-aware cleanup
Resource cleanup SHALL observe an active cancellation token and avoid starting new blocking work when cancelled.

#### Scenario: Cleanup is cancelled
- **WHEN** a `match with` scope is cancelled before cleanup begins
- **THEN** the close is either skipped or returns a cancellation-specific error

