# zirk-lexical-syntax Delta

## MODIFIED Requirements

### Requirement: String literals

The lexer SHALL recognize string literals delimited by double quotes or by
triple single quotes `'''…'''`, with escape sequences. The `'''` form is
multiline and follows the left-edge rule owned by `triple-quoted-strings`.

#### Scenario: Simple string

- **WHEN** `"Hello"` is tokenized
- **THEN** a string literal with content `Hola` is produced

#### Scenario: Triple-quoted string

- **WHEN** a `'''…'''` literal is tokenized
- **THEN** a string literal is produced with its content, escapes,
  interpolations and left-edge stripping resolved per
  `triple-quoted-strings`

#### Scenario: Escape sequences

- **WHEN** a string contains `\n`, `\t`, `\"`, or `\\`
- **THEN** the literal represents them as a newline, tab, quote, and backslash

#### Scenario: Unclosed string

- **WHEN** a string is not closed before the end of the line or the file
- **THEN** a diagnostic pointing to the string's opening is emitted
- **AND** the help indicates that the closing quote is missing

#### Scenario: Unknown escape

- **WHEN** a string contains an unrecognized escape sequence
- **THEN** a diagnostic pointing to the sequence is emitted

### Requirement: Interpolation in string literals

The lexer SHALL recognize `{ expression }` interpolation within a string
literal — `"…"` or `'''…'''` — with balanced braces, producing the literal
parts and the embedded expressions as distinct elements of the literal.

The same balancing rules SHALL apply to range operands such as
`0..{number}:2`, retaining the inner expression and its source span without
treating the braces as a collection block.

#### Scenario: Interpolated string

- **WHEN** `"value={value}"` is tokenized
- **THEN** the literal keeps the textual part and the embedded expression separately
- **AND** the braces are NOT part of the text

#### Scenario: Interpolated triple-quoted string

- **WHEN** a `'''…'''` literal contains `{expr}`
- **THEN** the same literal/expression part structure is produced as for
  `"…"` interpolation

#### Scenario: Nested braces

- **WHEN** an interpolation itself contains balanced braces
- **THEN** the interpolation closes at its matching brace, not at the first one

#### Scenario: Unclosed interpolation

- **WHEN** an interpolation does not close before the end of the literal
- **THEN** a diagnostic pointing to its opening is emitted

#### Scenario: Interpolated range bound

- **WHEN** the source contains `0..{number}:1`
- **THEN** the token stream retains the interpolated bound as an expression within the range
