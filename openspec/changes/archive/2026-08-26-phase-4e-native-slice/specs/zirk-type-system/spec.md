## MODIFIED Requirements

### Requirement: Memory and task type family
The type system SHALL define `Weak<T>`, `Pointer<T>`, `NativeSlice<T>`, `NativeSliceMut<T>`, `Task<T>`, `TaskSettlement<T>`, `Channel<T>`, synchronization types, and their capability constraints without exposing mandatory ownership or lifetime parameters.

#### Scenario: Await type is inferred
- **WHEN** an expression has type `Task<Result<User, LoadError>>`
- **THEN** awaiting it has type `Result<User, LoadError>`

#### Scenario: NativeSlice element type is ABI-safe only
- **WHEN** `NativeSlice<T>`/`NativeSliceMut<T>` is instantiated with an element type outside the ABI-safe subset (`Void`/`Boolean`/fixed-width `Int`/`UInt`/`Float32`/`Float64`/nested `Pointer<T>`)
- **THEN** the checker rejects the instantiation, naming the unsupported element type
