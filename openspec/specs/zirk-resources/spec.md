# zirk-resources Specification

## Purpose
TBD - created by archiving change document-errors-resources-and-permissions. Update Purpose after archive.
## Requirements
### Requirement: Match with closes successful acquisitions exactly once
The language SHALL define `Resource<E from Error>` with typed `close` and closed-state behavior. `match with` SHALL own each successful acquisition and close it exactly once on normal completion, return, exception, cancellation, or control transfer leaving the scope.

#### Scenario: Body returns early
- **WHEN** a `match with` success branch returns before its final statement
- **THEN** the owned resource closes before the return value leaves the scope

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

