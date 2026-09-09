## MODIFIED Requirements

### Requirement: Complete range and slice forms

The parser SHALL accept `start..end`, `start..=end`, descending bounds, colon steps (`start..end:step` and `start..=end:step`), and braced interpolated bounds/steps such as `0..{number}:2`. It SHALL reject the legacy `start..end..step`, `.step(distance)`, and `.reverse()` range-builder forms. Slice syntax `[start:end:step]` SHALL remain unchanged.

#### Scenario: Descending stepped range
- **WHEN** source contains `10..=0:-2`
- **THEN** it represents an inclusive descending range with step `-2`

#### Scenario: Braced expression bound
- **WHEN** source contains `0..{limit + 1}:2`
- **THEN** the expression is retained as the range end operand

### Requirement: Collection literals with range expansion

The parser SHALL accept `[...]` as a collection literal and SHALL permit range expressions among its elements. Range elements SHALL be distinguishable from indexing and slicing by primary-expression context. Constructor argument lists for `Array(...)` and `List(...)` SHALL also accept range expressions for expansion.

#### Scenario: Range-only literal
- **WHEN** source contains `[0..100]`
- **THEN** it represents one collection literal whose expanded elements are `0` through `99`

#### Scenario: Mixed literal
- **WHEN** source contains `[1, 5..8, 9]`
- **THEN** it represents the expanded sequence `1, 5, 6, 7, 9`
