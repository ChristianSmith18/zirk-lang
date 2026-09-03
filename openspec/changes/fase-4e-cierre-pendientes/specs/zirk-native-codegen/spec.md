## ADDED Requirements

### Requirement: Native code generation supports value-type field pointers
The LLVM backend SHALL emit `getelementptr` over `ValueLayout` for `PointerFromField` when the container is a pointer to a `record` or `value class`.

#### Scenario: PointerFromField over value class
- **WHEN** the IR contains `PointerFromField { object: Pointer<Value>, index: n }`
- **THEN** the backend emits a GEP using the `ValueLayout` of the value class, with no object header offset

#### Scenario: PointerFromField over object unchanged
- **WHEN** the IR contains `PointerFromField { object: Object(id), index: n }`
- **THEN** the backend continues to emit a GEP using the `ObjectLayout` with the usual object header offset

### Requirement: Native code generation supports string grapheme offset
The LLVM backend SHALL declare and call `zirk_str_grapheme_offset` to implement `StringGraphemeOffset`.

#### Scenario: StringGraphemeOffset emitted
- **WHEN** the IR contains `StringGraphemeOffset { string, index }`
- **THEN** the backend emits a call to `zirk_str_grapheme_offset` and branches on `-1` to the bounds-fail block
