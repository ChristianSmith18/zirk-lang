## ADDED Requirements

### Requirement: Memory and task type family
The type system SHALL define `Weak<T>`, `Pointer<T>`, `NativeSlice<T>`, `NativeSliceMut<T>`, `Task<T>`, `TaskSettlement<T>`, `Channel<T>`, synchronization types, and their capability constraints without exposing mandatory ownership or lifetime parameters.

#### Scenario: Await type is inferred
- **WHEN** an expression has type `Task<Result<User, LoadError>>`
- **THEN** awaiting it has type `Result<User, LoadError>`

### Requirement: Derived concurrent capabilities
Transferability and shareability SHALL be compiler-derived, non-forgeable properties based on the complete reachable type graph, mutability, resource ownership, and synchronization contract.

#### Scenario: Class contains mutex
- **WHEN** a class safely encapsulates mutable state behind a supported mutex
- **THEN** the compiler may derive sharing without exposing an ordinary user-implemented marker
