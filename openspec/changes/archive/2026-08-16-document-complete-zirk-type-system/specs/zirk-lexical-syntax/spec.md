## ADDED Requirements

### Requirement: Float and temporal literal vocabulary
The lexer SHALL recognize `Float*` type names and duration suffixes `ns`, `us`, `ms`, `s`, `m`, `h`, `d`, and `w` without treating `m` as a calendar month. Calendar months and years SHALL be expressed through `Period` constructors or ISO period strings.

#### Scenario: Duration suffixes
- **WHEN** source contains `10ns`, `500ms`, `2h`, or `3d`
- **THEN** each is emitted as a duration literal carrying its unit

#### Scenario: Period month is not a duration literal
- **WHEN** a developer needs one calendar month
- **THEN** the documented source form is `Period.months(1)` rather than an ambiguous duration suffix

### Requirement: Grapheme character literal
The lexer SHALL preserve the complete Unicode content of a quoted character literal for grapheme validation and SHALL NOT equate one `Char` with one byte or one code point.

#### Scenario: Family emoji character
- **WHEN** a quoted character literal contains one family emoji grapheme
- **THEN** the lexer emits one character literal for semantic grapheme validation
