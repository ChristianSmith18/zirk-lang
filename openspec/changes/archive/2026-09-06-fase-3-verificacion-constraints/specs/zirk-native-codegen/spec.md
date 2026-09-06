# Delta spec: zirk-native-codegen

## MODIFIED Requirements

### Requirement: Object layout

The backend SHALL translate a type with identity into a structure whose header precedes its fields, and whose inherited fields precede its own.

#### Scenario: Shared prefix
- **WHEN** a base class and a subclass are translated
- **THEN** the prefix of the subclass's structure matches that of the base

#### Scenario: Inline record field
- **WHEN** a `record` is a field of another declaration
- **THEN** it is translated without an intermediate pointer

### Requirement: Native code generation supports value-type field pointers
The LLVM backend SHALL emit `getelementptr` over `ValueLayout` for `PointerFromField` when the container is a pointer to a `record`.

#### Scenario: PointerFromField over record
- **WHEN** the IR contains `PointerFromField { object: Pointer<Value>, index: n }`
- **THEN** the backend emits a GEP using the `ValueLayout` of the record, with no object header offset

#### Scenario: PointerFromField over object unchanged
- **WHEN** the IR contains `PointerFromField { object: Object(id), index: n }`
- **THEN** the backend continues to emit a GEP using the `ObjectLayout` with the usual object header offset
