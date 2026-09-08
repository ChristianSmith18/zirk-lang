# Delta: exact-decimal-arithmetic

## RENAMED Requirements

- FROM: `### Requirement: \`BinaryFloat\` family carries the IEEE 754 binary semantics`
- TO: `### Requirement: \`Float\` family carries the IEEE 754 binary semantics`

## MODIFIED Requirements

### Requirement: `Float` family carries the IEEE 754 binary semantics

The type system SHALL recognize `Float16`, `Float32`,
`Float64`, and `Float128`, with `Float` aliasing
`Float64`. This family SHALL have exactly the semantics the former
`BinaryFloat16`/`BinaryFloat32`/`BinaryFloat64`/`BinaryFloat128` family had: IEEE 754 binary values at
each width, explicit `POSITIVE_INFINITY` and `NEGATIVE_INFINITY`, no valid
`NaN`, an operation that would produce `NaN` as a controlled runtime error at
its point of occurrence, `Float128` text rendering truncated to
`Float64` precision, and the pending Windows verification for
`Float128`. A `Float` literal SHALL be written with an `f` suffix
(`1.5f`), optionally with a width (`1.5f32`, `0.1f128`). The former `b`/`bN`
literal suffixes and the `BinaryFloat*` spellings SHALL NOT resolve; the
lexer SHALL emit a diagnostic pointing to `f`/`fN` and the checker SHALL emit
a diagnostic naming the `Float` replacement.

#### Scenario: Binary literal suffix

- **WHEN** `inmut x = 1.5f32;` is written
- **THEN** `x` has type `Float32`

#### Scenario: Old `b` literal suffix is rejected

- **WHEN** `inmut x = 1.5b32;` is written
- **THEN** a diagnostic states that the binary-float suffix is now `f`/`fN`

#### Scenario: Binary division by zero yields infinity

- **WHEN** a non-zero `Float64` is divided by `0.0f`
- **THEN** the result is an infinity, not an error

#### Scenario: Binary indeterminate operation is controlled

- **WHEN** a `Float` operation would produce `NaN` under IEEE 754
- **THEN** the program raises a catchable `FloatNanError` at that operation
