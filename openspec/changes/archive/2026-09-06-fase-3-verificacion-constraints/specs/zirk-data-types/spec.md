# Delta spec: zirk-data-types

## REMOVED Requirements

### Requirement: Value classes
**Reason**: `value class` was removed from the data model; `record` is the
nominal immutable value type and the recommended domain value type.
**Migration**: declare a `record` for value semantics, or a `class` when
identity or mutability is wanted.

## MODIFIED Requirements

### Requirement: Pointer.from accepts record lvalues
`Pointer.from(place)` SHALL accept a `place` that is an lvalue of a `record`
slot or field, provided the final pointee type is FFI-safe.

#### Scenario: Pointer.from(record field)
- **WHEN** the program contains `Pointer.from(record_instance.x)` inside `unsafe`
- **THEN** the compiler lowers it to a chain of pointer-to-field operations ending at `Pointer<T>`

#### Scenario: Pointer.from rejects temporary value
- **WHEN** the program contains `Pointer.from(Foo().x)` or `Pointer.from(makePoint().y)`
- **THEN** the compiler reports an error because the root is not an lvalue

#### Scenario: Pointer.from on nested record field
- **WHEN** the program contains `Pointer.from(point.inner.x)` where `inner` is a `record` field
- **THEN** the compiler builds a `Pointer<Value>` to `inner` and then `PointerFromField` to `x`
