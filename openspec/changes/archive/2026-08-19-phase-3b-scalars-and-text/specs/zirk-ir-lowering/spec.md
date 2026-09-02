## ADDED Requirements

### Requirement: Integer type parameterized by width and signedness in the IR

The IR SHALL represent every integer type with a single type parameterized by width and signedness, not with a distinct variant per width, so that an existing arithmetic or conversion instruction remains valid for any width without being duplicated.

#### Scenario: Width-independent arithmetic instruction
- **WHEN** an addition between two `Int8` operands is lowered
- **THEN** the same instruction shape as for `Int32` is emitted, with the width as type data, not a distinct instruction

### Requirement: Overflow check per width and signedness

Lowering SHALL emit the overflow check matching the operation's integer type's width and signedness, reusing the mechanism Phase 1 established for `Int32`.

#### Scenario: Checked overflow on a narrow width
- **WHEN** an arithmetic operation on `Int8` is lowered
- **THEN** the IR includes the overflow check for that width, not `Int32`'s

### Requirement: `NaN` as controlled failure, not as a propagated value

Lowering a `Float` operation that can produce `NaN` under the backend's semantics SHALL include the check that turns it into a controlled failure before the value is used.

#### Scenario: Potentially indeterminate floating-point division
- **WHEN** `a / b` on `Float64` is lowered without the checker being able to rule out `a == 0.0 && b == 0.0`
- **THEN** the IR includes the corresponding check before producing the result

### Requirement: Desugaring deep contextual conversion

Lowering SHALL insert the conversion of every operand of a contextual operator tree before lowering the operation itself, instead of lowering the operation with its original type and converting the result afterward.

#### Scenario: Division lowered with the context already applied
- **WHEN** `Float(3 / 4)` is lowered
- **THEN** the division's operands are already `Float64` in the resulting IR, not `Int32` converted afterward

### Requirement: Desugaring string interpolation

Lowering SHALL desugar a literal with interpolated expressions into a sequence of `to_string()` calls concatenated with the literal text, in order of appearance.

#### Scenario: Interpolation with one expression
- **WHEN** `"Hola, {nombre}"` is lowered
- **THEN** the resulting IR calls `to_string()` on `nombre` and concatenates it with the literal text
