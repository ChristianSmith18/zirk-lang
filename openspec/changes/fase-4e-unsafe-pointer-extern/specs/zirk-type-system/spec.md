## MODIFIED Requirements

### Requirement: Memory and task type family
The type system SHALL define `Weak<T>`, `Pointer<T>`, `NativeSlice<T>`, `NativeSliceMut<T>`, `Task<T>`, `TaskSettlement<T>`, `Channel<T>`, synchronization types, and their capability constraints without exposing mandatory ownership or lifetime parameters.

#### Scenario: Await type is inferred
- **WHEN** an expression has type `Task<Result<User, LoadError>>`
- **THEN** awaiting it has type `Result<User, LoadError>`

## ADDED Requirements

### Requirement: `Pointer<T>` is restricted to an ABI-stable element type

`Pointer<T>` SHALL only be constructed, named, or cast to when `T` is `Boolean`, a fixed-width `Int`/`UInt` type, `Float32`, `Float64`, or another ABI-stable `Pointer<U>` — the set with a stable C-compatible layout. `Byte` SHALL be a recognized alias of `UInt8`, matching its use as `Pointer<T>`'s element type in native-interoperability examples.

#### Scenario: Disallowed element type
- **WHEN** an annotation names `Pointer<String>` or `Pointer<SomeClass>`
- **THEN** compilation rejects it, naming the disallowed element type

#### Scenario: `Byte` resolves to `UInt8`
- **WHEN** source names the type `Byte`
- **THEN** it resolves to the same type as `UInt8`

### Requirement: `Pointer<T>` operations require an unsafe boundary and cannot escape their frame

`Pointer.from(place)`, `.read()`, `.write(value)`, `.offset(n)`, `.offset_bytes(n)`, and `.cast<U>()` SHALL require an enclosing `unsafe` boundary; `.is_null` MAY be evaluated outside one. A value of type `Pointer<T>` SHALL NOT be returned from a function, assigned to a class field, or captured by a closure.

#### Scenario: Pointer operation outside unsafe
- **WHEN** code calls `.read()` on a `Pointer<T>` outside an `unsafe` block
- **THEN** compilation fails identifying the missing unsafe boundary

#### Scenario: Pointer escapes as a return value
- **WHEN** a function returns a `Pointer<T>` value
- **THEN** compilation fails identifying the escape

#### Scenario: Pointer escapes into a field
- **WHEN** a constructor assigns a `Pointer<T>` value to a class field
- **THEN** compilation fails identifying the escape

### Requirement: `extern "C" fn` signatures are restricted to ABI-stable types and calls require unsafe and commit

Every parameter type and the return type of an `extern "C" fn` SHALL be `Void` (return only), `Boolean`, a fixed-width `Int`/`UInt` type, `Float32`, `Float64`, or an ABI-stable `Pointer<T>`. Calling an `extern "C" fn` SHALL require both an enclosing `unsafe` boundary and an enclosing `commit` boundary, without inferring whether the specific call is free of side effects.

#### Scenario: Disallowed extern parameter type
- **WHEN** an `extern "C" fn` declares a parameter of type `String`
- **THEN** compilation rejects the declaration, naming the disallowed type

#### Scenario: Extern call outside commit
- **WHEN** code calls an `extern "C" fn` inside `unsafe {}` but not inside a nested `commit {}`
- **THEN** compilation fails identifying the missing commit boundary

### Requirement: An unsafe block journals managed writes and rolls them back on failure before its own commit

An ordinary `unsafe {}` block SHALL journal writes it performs to pre-existing Zirk-managed storage (including through `Pointer<T>.write()`) and SHALL restore them, in reverse order, if a controlled `Error`, exception, or checked runtime trap becomes pending before the block's own normal completion or its own `commit {}`. Entering `commit {}` SHALL durably commit every write journaled by the enclosing block so far, before that region's own body executes.

#### Scenario: Rollback on exception before commit
- **WHEN** an `unsafe {}` block writes through a `Pointer<T>` and then an exception becomes pending before reaching `commit {}` or the block's end
- **THEN** the written storage is restored to its value from before the block, before the exception propagates

#### Scenario: Commit is durable even if the block later fails
- **WHEN** an `unsafe {}` block writes through a `Pointer<T>`, reaches `commit {}`, and a later part of the same block fails afterward
- **THEN** the write made before `commit {}` is not rolled back
