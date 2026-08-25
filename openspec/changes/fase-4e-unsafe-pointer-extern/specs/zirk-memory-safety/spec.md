## MODIFIED Requirements

### Requirement: Closed unsafe operation set
Raw pointer creation, dereference, arithmetic and representation casts; unsafe native calls; unchecked native construction; untagged native-union access; weak atomic ordering; and manual safety-contract implementation SHALL require an explicit unsafe boundary.

#### Scenario: Safe code dereferences a pointer
- **WHEN** code dereferences `Pointer<T>` outside an unsafe block
- **THEN** compilation fails and identifies the required unsafe operation

### Requirement: Pointer and native-view behavior
Pointer arithmetic SHALL be measured in elements, byte offsets SHALL be explicit, null pointers SHALL be permitted only as raw native values, and validated `NativeSlice<T>`/`NativeSliceMut<T>` views SHALL carry bounded extent and lifetime.

#### Scenario: Null raw pointer is inspected
- **WHEN** native code returns a null `Pointer<T>`
- **THEN** `is_null` can inspect it without creating a nullable safe reference

### Requirement: Transactional unsafe mutation
An ordinary unsafe block SHALL isolate and journal writes to Zirk-managed state and validated native ranges, SHALL commit them on success, and SHALL close newly acquired resources and roll them back on a controlled `Error`, exception, runtime trap, or cancellation before commit.

#### Scenario: Validation fails after managed writes
- **WHEN** an unsafe block mutates managed state and then propagates a validation `Error`
- **THEN** the managed state is restored to its pre-block value before the error escapes

### Requirement: Irreversible commit boundary
External I/O, unknown-effect FFI, volatile or device memory, manual release, concurrently observable publication, and raw writes without proven provenance and extent MUST occur inside an explicit `commit {}` region within unsafe code.

#### Scenario: Socket send appears in reversible region
- **WHEN** unsafe code attempts to send network bytes before an explicit commit boundary
- **THEN** compilation fails because the effect cannot be rolled back
