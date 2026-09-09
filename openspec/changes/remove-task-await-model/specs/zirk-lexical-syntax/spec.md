## MODIFIED Requirements

### Requirement: Reserved words of the whole language

The lexer SHALL recognize as keywords those of the whole language, not only those of the implemented subset. `task` and `await` are NOT keywords: they were removed from the language and the parser treats them, in a construct position, as removed constructs. `select`, `scope`, and `shield` are ordinary identifiers.

Recognizing whole-language keywords allows the parser to distinguish an unimplemented construct from a syntax error and makes the diagnostic understandable.

#### Scenario: Keyword outside the subset

- **WHEN** `class`, `for`, `match`, or another keyword of the whole language is tokenized
- **THEN** the corresponding keyword token is produced and no identifier is produced

#### Scenario: Removed word is an identifier

- **WHEN** `mut task = 1;` or `mut await = 2;` is tokenized
- **THEN** `task` and `await` are produced as identifiers

### Requirement: Correct phase attribution per token

Every keyword and every operator outside the implemented subset SHALL declare the phase the roadmap assigns to it. `task` and `await` SHALL NOT appear in the phase table, having been removed. `parallel` and `thread` remain, attributed to Phase 5.

#### Scenario: Error-handling keyword

- **WHEN** the phase of `default` is queried
- **THEN** it declares the phase of `try`/`catch`, not that of decorators

#### Scenario: Removed word has no phase

- **WHEN** the phase table is queried for `task`
- **THEN** `task` is absent from the table
