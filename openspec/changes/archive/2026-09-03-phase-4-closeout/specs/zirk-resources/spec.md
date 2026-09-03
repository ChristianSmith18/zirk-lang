## ADDED Requirements

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
