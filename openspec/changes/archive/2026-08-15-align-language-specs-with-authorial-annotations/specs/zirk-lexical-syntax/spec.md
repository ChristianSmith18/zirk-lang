## ADDED Requirements

### Requirement: Confirmed authorial tokens and literals
The lexer SHALL recognize `**` and `**=`, the `do`, `gen`, and `yield` words, the inclusive range token `..=`, and regex literals delimited as `re'pattern'`. Regex scanning SHALL preserve escapes for the regex parser and SHALL diagnose an unterminated literal at its opening delimiter.

#### Scenario: Exponentiation tokens
- **WHEN** source contains `value ** 2` or `value **= 2`
- **THEN** the lexer emits exponentiation and compound-exponentiation tokens rather than two multiplication tokens

#### Scenario: Regex literal
- **WHEN** source contains `re'^[0-9]+$'`
- **THEN** the lexer emits one typed regex literal with the pattern text

#### Scenario: Unterminated regex
- **WHEN** a regex literal reaches end of line or file without its closing quote
- **THEN** a lexical diagnostic points to the opening `re'`

### Requirement: Range interpolation tokens
The lexer SHALL preserve `{ expression }` interpolation embedded in an inline range bound such as `0..{number}` using the same balanced interpolation rules as other interpolated expressions.

#### Scenario: Computed range bound
- **WHEN** source contains `0..{number}.step(1)`
- **THEN** the token stream retains the interpolated bound as an expression inside the range
