## Why

The names `Float` and `BinaryFloatN` are semantically backwards relative to common programming vocabulary. The exact base-ten type is currently called `Float`, while the IEEE 754 binary family is called `BinaryFloatN`. This is confusing because `Float` is conventionally understood as a binary floating-point type, and `Decimal` is the conventional name for exact base-ten numbers. Renaming the types makes the language more self-documenting and lowers the cognitive load for new users.

## What Changes

- **BREAKING**: The exact base-ten type currently named `Float` is renamed to `Decimal`. The abbreviated form `Dec` is also accepted (e.g., `mut x: Dec = 0.1`).
- **BREAKING**: The IEEE 754 binary family currently named `BinaryFloatN` (`BinaryFloat16`, `BinaryFloat32`, `BinaryFloat64`, `BinaryFloat128`) is renamed to `FloatN` (`Float16`, `Float32`, `Float64`, `Float128`). The unqualified name `Float` in this family becomes an alias for `Float64`.
- The literal suffixes become `d` for `Decimal` (e.g., `1.5d`) and `f`/`fN` for `FloatN` (e.g., `1.5f`, `1.5f32`). The old `b`/`bN` suffixes are rejected with a diagnostic pointing to `f`/`fN`.
- The built-in method and member names (`Decimal.parse`, `Decimal.MIN`, `Float64.EPSILON`, etc.) follow the new type names.
- Diagnostics and error messages that mention `Float` / `BinaryFloat` are updated to the new names.
- Handbook, language spec, feature status, and built-in type reference are updated to describe `Decimal` as the recommended default fractional type and `FloatN` as the binary, hardware-oriented family.

## Capabilities

### New Capabilities

- None. This is a pure surface rename.

### Modified Capabilities

- `zirk-scalars`: Replaces the `Float` and `BinaryFloatN` scalar names with `Decimal`/`Dec` and `FloatN`. Updates member tables and static constants.
- `zirk-grammar`: Updates the grammar for type names, including the `Decimal`/`Dec` alias and the `FloatN` family.
- `zirk-lexical-syntax`: Changes fractional literal suffixes to `d` for `Decimal` and `f`/`fN` for `FloatN`; the old `b`/`bN` suffixes are rejected.
- `zirk-type-system`: Updates conversion rules, contextual literal inference, and type compatibility tables to use the new names.
- `zirk-data-types`: Updates the built-in type reference and the type categories documentation.

## Impact

- Breaking change to user-visible syntax and diagnostics. Existing `Float` and `BinaryFloat*` spellings become rejected with a clear diagnostic pointing to the new names.
- The compiler pipeline (lexer, parser, semantic checker, IR, codegen, runtime) is touched only to the extent of replacing identifiers and diagnostic strings. No runtime logic changes.
- Public documentation and the companion website must be regenerated and synced.
