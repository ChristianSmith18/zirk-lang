## ADDED Requirements

### Requirement: Codegen for exact-decimal `Float`

The backend SHALL emit an exact-decimal `Float` as an LLVM aggregate of a
128-bit integer coefficient and an integer scale, passed and returned across the
runtime boundary by pointer, in the same manner as `Int128` and `UInt128`. The
backend SHALL emit `Float` constants directly from the parsed coefficient and
scale, never routing through any binary floating type. The backend SHALL emit
calls to the decimal runtime helpers for arithmetic, comparison, rounding,
formatting, parsing, and conversion.

#### Scenario: Constant emitted without a binary intermediate

- **WHEN** code is emitted for the `Float` literal `0.1`
- **THEN** the aggregate `{ coefficient = 1, scale = 1 }` is emitted directly

#### Scenario: Arithmetic dispatched to the runtime

- **WHEN** code is emitted for `Float` addition
- **THEN** a call to the decimal-addition runtime symbol is emitted, with the
  operands passed by pointer

## MODIFIED Requirements

### Requirement: Codegen for `Float` over LLVM's native type

The backend SHALL emit each IR `BinaryFloat` type as the LLVM `FloatType` of the
corresponding width, with the `NaN`/invalid-domain check emitted explicitly
around the native operation, not delegated to the backend's floating-point
semantics. Exact-decimal `Float` SHALL NOT use an LLVM `FloatType`; it is
covered by the exact-decimal codegen requirement.

#### Scenario: Explicit check before the result

- **WHEN** code is emitted for a `BinaryFloat` operation the IR marked as
  potentially indeterminate
- **THEN** the generated code checks the condition before the result is used, and
  calls the runtime for the controlled failure if it holds

#### Scenario: Binary width maps to the native type

- **WHEN** code is emitted for a `BinaryFloat32` value
- **THEN** the LLVM `f32` type is used
