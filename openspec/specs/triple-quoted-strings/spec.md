# triple-quoted-strings Specification

## Purpose

Defines the `'''…'''` multiline string literal and its compile-time left edge.

## Requirements

### Requirement: Triple-quoted literal

The lexer SHALL recognize `'''…'''` as a `String`. Single `'` and double `''` runs inside content are literal; the first `'''` closes; `''''''` is empty.

#### Scenario: Quotes inside content

- **WHEN** content contains `it's` and `''quoted''`
- **THEN** both runs are text, not delimiters

### Requirement: Literal content preservation

Newlines, tabs, and spaces SHALL be preserved except for edge stripping. The newline immediately after the opening delimiter and the closing delimiter line do not contribute to the value.

#### Scenario: Blank lines

- **WHEN** content contains `a`, an empty line, and `b`
- **THEN** the value is `a\n\nb` with no trailing newline

### Requirement: Escapes and interpolation

The `'''` form SHALL honor the normal string escape set and balanced `{expr}` interpolation through `to_string()`.

#### Scenario: Literal brace escape

- **WHEN** content contains `\{`
- **THEN** it contributes a literal `{` without opening interpolation

### Requirement: Left-edge rule

The opening delimiter column, measured in characters with a tab counting as one, SHALL define the edge. Each non-empty content line MUST start at or right of it and has exactly that prefix stripped; blank lines are exempt.

#### Scenario: Extra indentation is content

- **WHEN** a line has two spaces beyond the edge
- **THEN** those two spaces remain in the value

#### Scenario: Line left of the edge

- **WHEN** a non-empty line starts left of the opening delimiter
- **THEN** a diagnostic names the edge column and rejects the literal

### Requirement: Closing delimiter placement and canonical layout

The closing `'''` MUST be on its own line at or right of the edge. Canonical layout aligns opening and closing delimiters one indent deeper than the introducing line, with content at that column; `zirk format` enforces it and the editor guide visualizes it.

#### Scenario: Unclosed literal

- **WHEN** EOF is reached without a closing `'''`
- **THEN** a diagnostic points at the opening delimiter

#### Scenario: Closing delimiter left of the edge

- **WHEN** the closing delimiter starts left of the edge
- **THEN** a diagnostic points at that delimiter
