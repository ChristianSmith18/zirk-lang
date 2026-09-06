## MODIFIED Requirements

### Requirement: Fractional literals and scientific notation

The lexer SHALL recognize fractional and scientific-notation literals, distinct
from an integer followed by a member access. A fractional or scientific literal
with no suffix SHALL be an exact-decimal `Float` literal. A fractional or
scientific literal with a `b` suffix (`1.5b`), optionally carrying a width
(`1.5b16`, `1.5b32`, `1.5b64`, `1.5b128`), SHALL be a `BinaryFloat` literal.
The literal text SHALL be carried verbatim for the semantic phase.

Without this rule, `1.5` tokenizes as `1`, `.`, and `5`, which is the worst way
to fail: the language cannot say "not yet" about something it does not even see.

#### Scenario: Fractional literal is exact decimal

- **WHEN** `1.5` is tokenized
- **THEN** a single exact-decimal `Float` literal is produced
- **AND** the sequence integer, dot, integer is NOT produced

#### Scenario: Scientific notation is exact decimal

- **WHEN** `6.02e23` or `1e2` is tokenized without a suffix
- **THEN** a single exact-decimal `Float` literal with its exponent is produced

#### Scenario: Binary-float suffix

- **WHEN** `1.5b32` is tokenized
- **THEN** a single `BinaryFloat` literal is produced and retains the requested
  width for the semantic check

#### Scenario: Bare binary-float suffix

- **WHEN** `0.1b` is tokenized
- **THEN** a `BinaryFloat64` literal is produced
