## MODIFIED Requirements

### Requirement: Interpolation in string literals

The lexer and parser SHALL apply balanced-brace handling to range operands as well as string interpolation. A range bound or step written as `{ expression }` SHALL retain the inner expression and its source span without treating the braces as a collection block.

#### Scenario: Interpolated range bound
- **WHEN** the source contains `0..{number}:2`
- **THEN** the token stream and parser retain `number` as the range end expression

#### Scenario: Nested range expression
- **WHEN** the source contains `0..{base + (offset * 2)}`
- **THEN** balanced braces delimit the complete expression

## ADDED Requirements

### Requirement: Colon range-step delimiter

The lexer SHALL emit the existing `Colon` token for a range step delimiter without merging it with `ColonColon` or changing ternary/type-colon tokenization.

#### Scenario: Colon step tokenization
- **WHEN** `0..10:-2` is tokenized
- **THEN** the stream contains `Integer`, `DotDot`, `Integer`, `Colon`, `Minus`, and `Integer` tokens in that order
