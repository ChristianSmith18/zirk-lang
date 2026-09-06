## MODIFIED Requirements

### Requirement: Integer-width and `Float` literals

The grammar SHALL recognize an integer literal as any of the signed or unsigned
widths when the context determines it. The grammar SHALL recognize a fractional
literal (with optional scientific notation) and no suffix as an exact-decimal
`Float` literal, and the same form with a `b`, `b16`, `b32`, `b64`, or `b128`
suffix as a `BinaryFloat` literal. `_` SHALL be accepted as a visual separator
in all numeric literals.

#### Scenario: Visual separator in a wide literal

- **WHEN** `1_000_000` is written
- **THEN** it is lexed as the integer `1000000`

#### Scenario: Scientific notation is exact

- **WHEN** `1e2` is written
- **THEN** it is lexed as an exact-decimal `Float` literal with value `100`

#### Scenario: Binary-float literal

- **WHEN** `1.5b32` is written
- **THEN** it is lexed as a `BinaryFloat32` literal

### Requirement: Contextual constructor expressions

The grammar SHALL retain the complete contained operator tree of
`Float(expression)`, `BinaryFloat(expression)`, and `String(expression)` so
semantic analysis can apply explicit deep contextual conversion before evaluating
compatible contained arithmetic or concatenation operators. `Float(expression)`
SHALL establish an exact-decimal domain; `BinaryFloat(expression)` SHALL
establish a binary domain.

#### Scenario: Nested contextual arithmetic

- **WHEN** `Float((a + 1) / (b * 2))` is parsed
- **THEN** the constructor contains the entire nested arithmetic tree rather than
  an already-evaluated integer result

#### Scenario: Binary contextual constructor

- **WHEN** `BinaryFloat(3 / 4)` is parsed
- **THEN** the constructor retains the division tree for a binary-domain conversion
