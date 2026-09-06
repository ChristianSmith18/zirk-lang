## ADDED Requirements

### Requirement: Lowering of exact-decimal `Float` operations

Lowering SHALL represent an exact-decimal `Float` as a distinct IR type carrying
a 128-bit coefficient and a decimal scale, and SHALL carry a `Float` literal as
verbatim text parsed no earlier than the runtime/codegen boundary. Arithmetic,
comparison, rounding, and conversion on `Float` that requires scale alignment,
normalization, or rounding SHALL lower to dedicated runtime calls. A `Float`
division or modulo SHALL lower with a zero-divisor guard that raises
`DivisionByZeroError` before the runtime call, in the same way `Duration`
division is lowered.

#### Scenario: Exact addition lowers to a runtime call

- **WHEN** `0.1 + 0.2` on `Float` is lowered
- **THEN** the IR contains a call to the decimal-addition runtime helper with the
  two operand values, not an LLVM binary `fadd`

#### Scenario: Divide-by-zero guard precedes the call

- **WHEN** `a / b` on `Float` is lowered
- **THEN** the IR raises `DivisionByZeroError` when `b` is zero, before the
  decimal-division helper is invoked

#### Scenario: Integer operand converted to exact `Float`

- **WHEN** `Int32 + Float` is lowered
- **THEN** the `Int32` operand is converted to an exact `Float` at scale 0 before
  the addition

## MODIFIED Requirements

### Requirement: `NaN` as a controlled failure, not as a propagated value

Lowering SHALL include, for a `BinaryFloat` operation that can produce `NaN`
under the backend's semantics, the check that turns it into a controlled failure
before the value is used. Exact-decimal `Float` operations SHALL NOT emit a
`NaN` check
because `Float` has no `NaN`; instead they SHALL emit the coefficient-overflow
and zero-divisor checks appropriate to exact decimal arithmetic.

#### Scenario: Potentially indeterminate binary division

- **WHEN** `a / b` on `BinaryFloat64` is lowered without the checker being able
  to rule out `a == 0.0b && b == 0.0b`
- **THEN** the IR includes the corresponding check before producing the result

#### Scenario: Exact division does not emit a `NaN` check

- **WHEN** `a / b` on `Float` is lowered
- **THEN** the IR emits a zero-divisor guard and no `NaN` check

### Requirement: Desugaring of deep contextual conversion

Lowering SHALL insert the conversion of each operand of a contextual operator
tree before lowering the operation itself, instead of lowering the operation with
its original type and converting the result afterward. `Float(expr)` SHALL
convert operands to exact decimal; `BinaryFloat(expr)` SHALL convert operands to
binary.

#### Scenario: Division lowered with the exact context already applied

- **WHEN** `Float(3 / 4)` is lowered
- **THEN** the operands of the division are already exact `Float` in the
  resulting IR, and the result is `0.75`

#### Scenario: Division lowered with the binary context already applied

- **WHEN** `BinaryFloat(3 / 4)` is lowered
- **THEN** the operands of the division are already `BinaryFloat64` in the
  resulting IR
