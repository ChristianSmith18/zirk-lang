# zirk-ir-lowering

## ADDED Requirements

### Requirement: Arithmetic operands are promoted to a common numeric type

When a binary arithmetic operation has operands of different numeric widths or families, lowering SHALL emit a widening conversion for each operand so that both have the same common type before the operation is performed. The common type is the smallest type that can represent every value of both operand types losslessly. For `++` and `--`, the literal `1` / `1.0` SHALL be created at the operand's exact type, so no promotion is needed.

#### Scenario: Integer operands of different widths
- **WHEN** `Int8 + Int32` is lowered
- **THEN** the `Int8` operand is widened to `Int32` before the addition

#### Scenario: Unsigned integer widened to a wider signed integer
- **WHEN** `UInt8 + Int32` is lowered
- **THEN** the `UInt8` operand is zero-extended to `Int32` before the addition

#### Scenario: Integer widened to a common float
- **WHEN** `Int32 + Float64` is lowered
- **THEN** the `Int32` operand is converted to `Float64` before the addition

#### Scenario: Unsafe integer-to-float conversion is rejected
- **WHEN** `Int32 + Float16` is lowered
- **THEN** compilation fails before lowering because `Float16` cannot losslessly represent every `Int32` value

#### Scenario: String repetition count widened
- **WHEN** `"x" * (3 as UInt8)` is lowered
- **THEN** the count is widened to `Int32` before the runtime call

#### Scenario: Increment uses the operand's own width
- **WHEN** `i++` on `Int8` is lowered
- **THEN** the constant `1` is created as `Int8` and no widening is emitted

## MODIFIED Requirements

### Requirement: Integer type parameterized by width and signedness in the IR

The IR SHALL represent each integer type with a single type parameterized by width and signedness, not with a distinct variant per width, so that an existing arithmetic or conversion instruction remains valid for any width without being duplicated. Widening and promotion instructions for mixed-width and mixed-family arithmetic SHALL use the same parameterised type forms.

#### Scenario: Arithmetic instruction independent of width
- **WHEN** an addition between two `Int8` operands is lowered
- **THEN** the same instruction form as for `Int32` is emitted, with the width as data of the type, not a distinct instruction

#### Scenario: Widening instruction is parameterised by source and target widths
- **WHEN** an `Int8` operand is promoted to `Int32` before a binary operation
- **THEN** the widening instruction carries both source and target width/signedness parameters, not new opcodes
