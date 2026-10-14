## ADDED Requirements

### Requirement: Ordering comparisons accept mixed numeric types

The comparison operators `<`, `<=`, `>`, `>=` SHALL accept operands of any two numeric types — every integer width and signedness, `Decimal`, and the `Float` family — and SHALL evaluate the comparison in the smallest common numeric type that can represent every value of both operand types, following the same widening lattice as arithmetic. When no common type can represent both operands without loss, the program SHALL be rejected at compile time. `Decimal` SHALL be orderable: `expect_comparable`-style checks SHALL accept `Decimal` alongside integers, `Float`, `Duration`, `Date`, `Time`, and `DateTime`.

#### Scenario: Signed counter against unsigned length
- **WHEN** `for mut i = 0; i < xs.length; i++` compares an `Int32` counter with a `UInt64` length
- **THEN** the comparison is evaluated in the smallest common type (`Int128`) and the loop compiles and runs

#### Scenario: Decimal ordering
- **WHEN** `price > qty` compares two `Decimal` values
- **THEN** the checker accepts the expression and the result is the exact decimal ordering

#### Scenario: Integer against Decimal
- **WHEN** an `Int32` is compared with a `Decimal`
- **THEN** the integer widens losslessly to `Decimal` and the comparison is exact

#### Scenario: Unsafe mixed-family comparison is rejected
- **WHEN** a `Decimal` and a `Float64` are compared without an explicit conversion
- **THEN** compilation fails, because no common type represents both exactly
