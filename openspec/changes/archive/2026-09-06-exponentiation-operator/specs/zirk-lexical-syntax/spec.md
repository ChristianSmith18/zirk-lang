## MODIFIED Requirements

### Requirement: Power tokens

The lexer SHALL recognize `**` and `**=` as their own tokens, applying the
longest-match rule before multiplication. These tokens belong to the
implemented subset: the lexer SHALL NOT attribute an arrival phase to them
and the compiler SHALL NOT defer them with a phase diagnostic.

#### Scenario: Power

- **WHEN** `value ** 2` or `value **= 2` is tokenized
- **THEN** the power and compound-power tokens are produced
- **AND** two consecutive multiplication tokens are NOT produced

#### Scenario: Power tokens carry no arrival phase

- **WHEN** the phase of `**` or `**=` is queried
- **THEN** it reports that the token is part of the implemented subset, not a
  later phase
