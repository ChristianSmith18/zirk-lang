## ADDED Requirements

### Requirement: Decimal exponentiation does not narrow exponents

Exact-decimal exponentiation SHALL preserve the full declared exponent value or report a controlled range/resource failure. It SHALL NOT truncate or wrap an exponent to a narrower integer type. Intermediate multiplication and result scale SHALL be checked for representability.

#### Scenario: Exponent is wider than the implementation word
- **WHEN** a valid integer exponent exceeds the range of a machine-word exponent
- **THEN** exponentiation computes according to the specified result or reports a controlled unsupported-range failure without silently changing the exponent

#### Scenario: Decimal power overflows its representation
- **WHEN** an intermediate coefficient or scale cannot be represented
- **THEN** the operation returns the documented controlled arithmetic failure

### Requirement: Decimal magnitude decisions remain exact

Operations whose rounding or scale decision depends on decimal magnitude SHALL use exact integer/decimal comparisons rather than converting the operand to a binary floating approximation.

#### Scenario: Division operands are near a decimal digit boundary
- **WHEN** division chooses precision for operands near a power-of-ten boundary
- **THEN** the chosen scale is correct for the exact decimal operands
