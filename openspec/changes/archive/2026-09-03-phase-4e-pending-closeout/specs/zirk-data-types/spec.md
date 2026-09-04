## ADDED Requirements

### Requirement: Pointer.from accepts record and value class lvalues
`Pointer.from(place)` SHALL accept a `place` that is an lvalue of a `record` or `value class` slot or field, provided the final pointee type is FFI-safe.

#### Scenario: Pointer.from(record field)
- **WHEN** the program contains `Pointer.from(record_instance.x)` inside `unsafe`
- **THEN** the compiler lowers it to a chain of pointer-to-field operations ending at `Pointer<T>`

#### Scenario: Pointer.from(value class field)
- **WHEN** the program contains `Pointer.from(value_class_instance.y)` inside `unsafe`
- **THEN** the compiler lowers it to a pointer into the inline value-type storage

#### Scenario: Pointer.from rejects temporary value
- **WHEN** the program contains `Pointer.from(Foo().x)` or `Pointer.from(makePoint().y)`
- **THEN** the compiler reports an error because the root is not an lvalue

#### Scenario: Pointer.from on nested record field
- **WHEN** the program contains `Pointer.from(point.inner.x)` where `inner` is a `record` field
- **THEN** the compiler builds a `Pointer<Value>` to `inner` and then `PointerFromField` to `x`
