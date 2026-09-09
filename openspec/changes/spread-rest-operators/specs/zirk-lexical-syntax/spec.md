## ADDED Requirements

### Requirement: Contextual `...` token use

The lexer SHALL continue emitting `DotDotDot` for `...`; parser context SHALL distinguish a variadic parameter marker, a spread expression, and a rest-pattern marker without introducing a second token.

#### Scenario: Spread tokenization
- **WHEN** `sum(...values)` is tokenized
- **THEN** `DotDotDot` precedes the identifier `values`

#### Scenario: Object spread tokenization
- **WHEN** `{ ...profile, name: "Grace" }` is tokenized
- **THEN** `DotDotDot` precedes `profile` and the braces remain available for contextual object parsing
